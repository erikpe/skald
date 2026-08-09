//! Source-to-assembly orchestration over explicit compiler phase boundaries.

use std::path::Path;

use crate::{
    backend::{emit_assembly, BackendError, Target},
    diagnostics::{Diagnostic, Diagnostics},
    lexer::lex,
    mir::{lower_preliminary_hir, verify_preliminary_mir},
    module::{
        load_module_graph, normalize_provider_roots, ModuleGraph, ProviderNormalizationError,
    },
    passes::{
        run_mir_pipeline,
        static_lifecycle::{plan_static_lifetimes, verify_planned_mir},
    },
    resolve::{resolve, resolve_module_graph, ResolvedProgram},
    source::SourceDatabase,
    syntax::parse,
    typeck::type_check,
};

use super::CompilationRequest;

/// Temporary driver boundary while verified static lifecycle MIR awaits
/// coordinator synthesis.
pub const STATIC_INITIALIZER_REQUIRES_LIFECYCLE_SYNTHESIS: &str = "DRV001";

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

    compile_module_graph_to_assembly(graph, request.target())
}

/// Compiles one in-memory singleton source through the same semantic and
/// backend pipeline as request compilation, without filesystem discovery.
pub fn compile_source_to_assembly(
    path: impl AsRef<Path>,
    text: impl Into<String>,
    target: Target,
) -> Result<AssemblyArtifact, CompilationError> {
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

    let resolved = resolve(&parsed.ast);
    diagnostics.append(resolved.diagnostics);
    finish_compilation(sources, diagnostics, resolved.program, target)
}

fn compile_module_graph_to_assembly(
    graph: ModuleGraph,
    target: Target,
) -> Result<AssemblyArtifact, CompilationError> {
    let resolved = resolve_module_graph(&graph);
    let sources = graph.into_sources();
    finish_compilation(sources, resolved.diagnostics, resolved.program, target)
}

fn finish_compilation(
    sources: SourceDatabase,
    mut diagnostics: Diagnostics,
    resolved: ResolvedProgram,
    target: Target,
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
    for initializer in planned.static_initializers() {
        diagnostics.push(
            Diagnostic::error(
                STATIC_INITIALIZER_REQUIRES_LIFECYCLE_SYNTHESIS,
                "static field lifecycle coordinator cannot be synthesized yet",
            )
            .with_primary_label(
                initializer.span,
                "lifecycle MIR and its effect certificate are verified, but coordinator synthesis is not implemented",
            )
            .with_note("verified initializer MIR cannot be consumed by a backend before coordinator synthesis"),
        );
    }
    if diagnostics.has_errors() {
        return Err(diagnostic_failure(sources, diagnostics));
    }
    let mir = planned
        .try_into_final()
        .expect("initializer-free planned MIR must convert to final MIR");
    let mir = run_mir_pipeline(mir).map_err(CompilationError::MirVerification)?;
    let assembly = emit_assembly(target, &mir).map_err(CompilationError::Backend)?;

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
