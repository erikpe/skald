//! Source-to-assembly orchestration over explicit compiler phase boundaries.

use std::path::Path;

use crate::{
    backend::{emit_assembly, BackendError, BackendInput, RuntimeTracePolicy, Target},
    diagnostics::Diagnostics,
    lexer::lex,
    mir::{lower_preliminary_hir, validate_hir_lowering_support, verify_preliminary_mir},
    module::{
        load_module_graph, normalize_provider_roots, ModuleGraph, ProviderNormalizationError,
    },
    passes::{
        run_mir_pipeline,
        static_lifecycle::{
            plan_static_lifetimes, synthesize_static_lifecycle, verify_planned_mir,
        },
    },
    resolve::{resolve_module_graph, resolve_with_source_path, ResolvedProgram},
    source::SourceDatabase,
    syntax::parse,
    typeck::type_check,
};

use super::CompilationRequest;

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
    let providers = normalize_provider_roots(
        request.environment().working_directory(),
        &request.provider_root_configurations(),
    )
    .map_err(CompilationError::ProviderConfiguration)?;
    let graph = load_module_graph(
        request.entry(),
        request.environment().working_directory(),
        &providers,
    )
    .map_err(|failure| {
        let (sources, diagnostics) = failure.into_parts();
        diagnostic_failure(sources, diagnostics)
    })?;

    compile_module_graph_to_assembly(graph, request.target(), request.runtime_trace())
}

/// Compiles one in-memory singleton source through the same semantic and
/// backend pipeline as request compilation, without filesystem discovery.
pub fn compile_source_to_assembly(
    path: impl AsRef<Path>,
    text: impl Into<String>,
    target: Target,
) -> Result<AssemblyArtifact, CompilationError> {
    let path = path.as_ref();
    let mut sources = SourceDatabase::new();
    let source_id = sources.add(path, text);
    let mut diagnostics = Diagnostics::new();

    let lexed = lex(sources.get(source_id).expect("source was just inserted"));
    diagnostics.append(lexed.diagnostics);
    if diagnostics.has_errors() {
        return Err(diagnostic_failure(sources, diagnostics));
    }

    let parsed = parse(
        sources.get(source_id).expect("source was just inserted"),
        &lexed.tokens,
    );
    diagnostics.append(parsed.diagnostics);
    if diagnostics.has_errors() {
        return Err(diagnostic_failure(sources, diagnostics));
    }

    let resolved = resolve_with_source_path(&parsed.ast, path);
    diagnostics.append(resolved.diagnostics);
    finish_compilation(
        sources,
        diagnostics,
        resolved.program,
        target,
        RuntimeTracePolicy::Enabled,
    )
}

fn compile_module_graph_to_assembly(
    graph: ModuleGraph,
    target: Target,
    runtime_trace: RuntimeTracePolicy,
) -> Result<AssemblyArtifact, CompilationError> {
    let resolved = resolve_module_graph(&graph);
    let sources = graph.into_sources();
    finish_compilation(
        sources,
        resolved.diagnostics,
        resolved.program,
        target,
        runtime_trace,
    )
}

fn finish_compilation(
    sources: SourceDatabase,
    mut diagnostics: Diagnostics,
    resolved: ResolvedProgram,
    target: Target,
    runtime_trace: RuntimeTracePolicy,
) -> Result<AssemblyArtifact, CompilationError> {
    if diagnostics.has_errors() {
        return Err(diagnostic_failure(sources, diagnostics));
    }

    let checked = type_check(&resolved);
    diagnostics.append(checked.diagnostics);
    if diagnostics.has_errors() {
        return Err(diagnostic_failure(sources, diagnostics));
    }
    let hir = checked
        .hir
        .expect("type checking without errors must produce typed HIR");
    diagnostics.append(validate_hir_lowering_support(&hir));
    if diagnostics.has_errors() {
        return Err(diagnostic_failure(sources, diagnostics));
    }
    let preliminary = lower_preliminary_hir(&hir);
    verify_preliminary_mir(&preliminary).map_err(CompilationError::MirVerification)?;
    let planned = match plan_static_lifetimes(preliminary) {
        Ok(planned) => planned,
        Err(failure) => {
            diagnostics.append(failure.into_diagnostics());
            return Err(diagnostic_failure(sources, diagnostics));
        }
    };
    verify_planned_mir(&planned).map_err(CompilationError::MirVerification)?;
    let mir = synthesize_static_lifecycle(planned).map_err(CompilationError::MirVerification)?;
    let mir = run_mir_pipeline(mir).map_err(CompilationError::MirVerification)?;
    let input = match runtime_trace {
        RuntimeTracePolicy::Enabled => BackendInput::with_runtime_trace(&mir, &sources),
        RuntimeTracePolicy::Omitted => BackendInput::without_runtime_trace(&mir),
    };
    let assembly = emit_assembly(target, input).map_err(CompilationError::Backend)?;

    Ok(AssemblyArtifact {
        assembly,
        report: CompilationReport {
            sources,
            diagnostics,
        },
    })
}

fn diagnostic_failure(sources: SourceDatabase, diagnostics: Diagnostics) -> CompilationError {
    CompilationError::Diagnostics(CompilationReport {
        sources,
        diagnostics,
    })
}
