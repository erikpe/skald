//! Source-to-assembly orchestration over explicit compiler phase boundaries.

use std::path::Path;

use crate::{
    backend::{emit_assembly, BackendError, BackendInput, RuntimeTracePolicy, Target},
    diagnostics::Diagnostics,
    lexer::lex,
    mir::{lower_preliminary_hir, verify_preliminary_mir},
    module::{
        load_module_graph_measured, normalize_provider_roots, ModuleGraph,
        ModuleLoadMeasurementOptions, ProviderNormalizationError,
    },
    passes::{
        run_mir_pipeline_measured,
        static_lifecycle::{
            plan_static_lifetimes, synthesize_static_lifecycle, verify_planned_mir,
        },
    },
    reporting::{
        NoopObserver, ReportDetail, ReportObserver, ReportOutcome, ReportPhase, ReportScope,
    },
    resolve::{resolve_module_graph, resolve_with_source_path, ResolvedProgram},
    source::SourceDatabase,
    syntax::parse,
    typeck::type_check,
};

use super::{
    observation::{observe_phase, observe_phase_with_metrics, observe_run},
    statistics, CompilationRequest,
};

#[derive(Debug)]
pub struct CompilationReport {
    pub sources: SourceDatabase,
    pub diagnostics: Diagnostics,
}

#[derive(Debug)]
pub struct AssemblyArtifact {
    pub assembly: String,
    pub report: CompilationReport,
}

#[derive(Debug)]
pub enum CompilationError {
    ProviderConfiguration(Vec<ProviderNormalizationError>),
    Diagnostics(CompilationReport),
    MirVerification(crate::mir::MirVerificationErrors),
    Backend(BackendError),
}

/// Loads and compiles one request's complete reachable module program.
///
/// Filesystem source acquisition remains in module loading. Artifact
/// publication remains a separate driver responsibility.
pub fn compile_request_to_assembly(
    request: &CompilationRequest,
) -> Result<AssemblyArtifact, CompilationError> {
    let mut observer = NoopObserver;
    compile_request_to_assembly_observed(request, &mut observer)
}

/// Loads and compiles a request while emitting typed phase observations.
///
/// The observer is invocation-local and does not become part of the request or
/// any compiler product. Host linking and artifact publication remain outside
/// this compilation-scoped API.
pub fn compile_request_to_assembly_observed(
    request: &CompilationRequest,
    observer: &mut dyn ReportObserver,
) -> Result<AssemblyArtifact, CompilationError> {
    observe_run(
        observer,
        ReportScope::Compilation,
        |observer| {
            let providers = observe_phase(
                observer,
                ReportPhase::ProviderNormalization,
                || {
                    let configurations = request.provider_root_configurations();
                    normalize_provider_roots(
                        request.environment().working_directory(),
                        &configurations,
                    )
                },
                result_outcome,
            )
            .map_err(CompilationError::ProviderConfiguration)?;
            let measurement_options = ModuleLoadMeasurementOptions::new(
                observer.enabled(ReportDetail::Details),
                observer.enabled(ReportDetail::Trace),
            );
            let loaded = observe_phase_with_metrics(
                observer,
                ReportPhase::ModuleLoading,
                || {
                    load_module_graph_measured(
                        request.entry(),
                        request.environment().working_directory(),
                        &providers,
                        measurement_options,
                    )
                },
                |measured| result_outcome(&measured.result),
                statistics::module_loading_metrics,
            );
            let graph = loaded.result.map_err(|failure| {
                let (sources, diagnostics) = failure.into_parts();
                diagnostic_failure(sources, diagnostics)
            })?;

            compile_module_graph_to_assembly(
                graph,
                request.target(),
                request.runtime_trace(),
                observer,
            )
        },
        result_outcome,
    )
}

/// Compiles one in-memory singleton source through the same semantic and
/// backend pipeline as request compilation, without filesystem discovery.
pub fn compile_source_to_assembly(
    path: impl AsRef<Path>,
    text: impl Into<String>,
    target: Target,
) -> Result<AssemblyArtifact, CompilationError> {
    let mut observer = NoopObserver;
    compile_source_to_assembly_observed(path, text, target, &mut observer)
}

/// Compiles one in-memory source while emitting typed phase observations.
///
/// Lexing and parsing are explicit phases for this adapter because no module
/// loader owns its frontend work.
pub fn compile_source_to_assembly_observed(
    path: impl AsRef<Path>,
    text: impl Into<String>,
    target: Target,
    observer: &mut dyn ReportObserver,
) -> Result<AssemblyArtifact, CompilationError> {
    let path = path.as_ref();
    observe_run(
        observer,
        ReportScope::Compilation,
        |observer| {
            let mut sources = SourceDatabase::new();
            let source_id = sources.add(path, text);
            let mut diagnostics = Diagnostics::new();

            let source_bytes = sources
                .get(source_id)
                .expect("source was just inserted")
                .len();
            let lexed = observe_phase_with_metrics(
                observer,
                ReportPhase::Lexing,
                || lex(sources.get(source_id).expect("source was just inserted")),
                |output| diagnostics_outcome(&output.diagnostics),
                |output, _| statistics::lexing_metrics(output, source_bytes),
            );
            diagnostics.append(lexed.diagnostics);
            if diagnostics.has_errors() {
                return Err(diagnostic_failure(sources, diagnostics));
            }

            let parsed = observe_phase_with_metrics(
                observer,
                ReportPhase::Parsing,
                || {
                    parse(
                        sources.get(source_id).expect("source was just inserted"),
                        &lexed.tokens,
                    )
                },
                |output| diagnostics_outcome(&output.diagnostics),
                |output, _| statistics::parsing_metrics(output, lexed.tokens.len()),
            );
            diagnostics.append(parsed.diagnostics);
            if diagnostics.has_errors() {
                return Err(diagnostic_failure(sources, diagnostics));
            }

            let resolved = observe_phase_with_metrics(
                observer,
                ReportPhase::Resolution,
                || resolve_with_source_path(&parsed.ast, path),
                |output| diagnostics_outcome(&output.diagnostics),
                |output, _| statistics::resolution_metrics(&output.program, &output.diagnostics),
            );
            diagnostics.append(resolved.diagnostics);
            finish_compilation(
                sources,
                diagnostics,
                resolved.program,
                target,
                RuntimeTracePolicy::Enabled,
                observer,
            )
        },
        result_outcome,
    )
}

fn compile_module_graph_to_assembly(
    graph: ModuleGraph,
    target: Target,
    runtime_trace: RuntimeTracePolicy,
    observer: &mut dyn ReportObserver,
) -> Result<AssemblyArtifact, CompilationError> {
    let resolved = observe_phase_with_metrics(
        observer,
        ReportPhase::Resolution,
        || resolve_module_graph(&graph),
        |output| diagnostics_outcome(&output.diagnostics),
        |output, _| statistics::resolution_metrics(&output.program, &output.diagnostics),
    );
    let sources = graph.into_sources();
    finish_compilation(
        sources,
        resolved.diagnostics,
        resolved.program,
        target,
        runtime_trace,
        observer,
    )
}

fn finish_compilation(
    sources: SourceDatabase,
    mut diagnostics: Diagnostics,
    resolved: ResolvedProgram,
    target: Target,
    runtime_trace: RuntimeTracePolicy,
    observer: &mut dyn ReportObserver,
) -> Result<AssemblyArtifact, CompilationError> {
    if diagnostics.has_errors() {
        return Err(diagnostic_failure(sources, diagnostics));
    }

    let checked = observe_phase_with_metrics(
        observer,
        ReportPhase::TypeChecking,
        || type_check(&resolved),
        |output| diagnostics_outcome(&output.diagnostics),
        |output, _| statistics::type_checking_metrics(output),
    );
    diagnostics.append(checked.diagnostics);
    if diagnostics.has_errors() {
        return Err(diagnostic_failure(sources, diagnostics));
    }
    let hir = checked
        .hir
        .expect("type checking without errors must produce typed HIR");
    let preliminary = observe_phase_with_metrics(
        observer,
        ReportPhase::PreliminaryMirLowering,
        || lower_preliminary_hir(&hir),
        |_| ReportOutcome::Completed,
        |program, _| statistics::preliminary_mir_metrics(program),
    );
    observe_phase_with_metrics(
        observer,
        ReportPhase::PreliminaryMirVerification,
        || verify_preliminary_mir(&preliminary),
        result_outcome,
        |result, _| statistics::verification_metrics(result),
    )
    .map_err(CompilationError::MirVerification)?;
    let planned = match observe_phase_with_metrics(
        observer,
        ReportPhase::StaticLifecyclePlanning,
        || plan_static_lifetimes(preliminary),
        result_outcome,
        |result, _| statistics::lifecycle_planning_metrics(result),
    ) {
        Ok(planned) => planned,
        Err(failure) => {
            diagnostics.append(failure.into_diagnostics());
            return Err(diagnostic_failure(sources, diagnostics));
        }
    };
    observe_phase_with_metrics(
        observer,
        ReportPhase::PlannedMirVerification,
        || verify_planned_mir(&planned),
        result_outcome,
        |result, _| statistics::verification_metrics(result),
    )
    .map_err(CompilationError::MirVerification)?;
    let mir = observe_phase_with_metrics(
        observer,
        ReportPhase::StaticLifecycleSynthesis,
        || synthesize_static_lifecycle(planned),
        result_outcome,
        |result, _| statistics::lifecycle_synthesis_metrics(result),
    )
    .map_err(CompilationError::MirVerification)?;
    let measured_pipeline = observe_phase_with_metrics(
        observer,
        ReportPhase::MirPipeline,
        || run_mir_pipeline_measured(mir),
        |measured| result_outcome(&measured.result),
        |measured, _| statistics::mir_pipeline_metrics(measured),
    );
    let mir = measured_pipeline
        .result
        .map_err(CompilationError::MirVerification)?;
    let assembly = observe_phase_with_metrics(
        observer,
        ReportPhase::BackendEmission,
        || {
            let input = match runtime_trace {
                RuntimeTracePolicy::Enabled => BackendInput::with_runtime_trace(&mir, &sources),
                RuntimeTracePolicy::Omitted => BackendInput::without_runtime_trace(&mir),
            }
            .with_reachable_artifacts_only();
            emit_assembly(target, input)
        },
        result_outcome,
        |result, _| statistics::backend_metrics(result),
    )
    .map_err(CompilationError::Backend)?;

    Ok(AssemblyArtifact {
        assembly,
        report: CompilationReport {
            sources,
            diagnostics,
        },
    })
}

fn result_outcome<T, E>(result: &Result<T, E>) -> ReportOutcome {
    if result.is_ok() {
        ReportOutcome::Completed
    } else {
        ReportOutcome::Failed
    }
}

fn diagnostics_outcome(diagnostics: &Diagnostics) -> ReportOutcome {
    if diagnostics.has_errors() {
        ReportOutcome::Failed
    } else {
        ReportOutcome::Completed
    }
}

fn diagnostic_failure(sources: SourceDatabase, diagnostics: Diagnostics) -> CompilationError {
    CompilationError::Diagnostics(CompilationReport {
        sources,
        diagnostics,
    })
}
