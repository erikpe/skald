//! Source-to-assembly orchestration over explicit compiler phase boundaries.

use std::path::Path;

use crate::{
    backend::{emit_assembly, BackendError, Target},
    diagnostics::{Diagnostic, Diagnostics},
    lexer::lex,
    mir::lower_hir,
    module::{
        load_module_graph, normalize_provider_roots, ModuleGraph, ProviderNormalizationError,
    },
    passes::run_mir_pipeline,
    resolve::{resolve, resolve_module_graph, ResolvedProgram},
    source::SourceDatabase,
    syntax::parse,
    typeck::{type_check, STATIC_STORAGE_NOT_LOWERED},
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
    for class in hir.classes.iter() {
        for field in &class.static_fields {
            diagnostics.push(
                Diagnostic::error(
                    STATIC_STORAGE_NOT_LOWERED,
                    format!(
                        "static field `{}.{}` cannot be emitted yet",
                        class.name, field.name
                    ),
                )
                .with_primary_label(
                    field.static_span,
                    "static MIR roots and target storage are not implemented",
                )
                .with_note("typed static places are available through HIR only for now"),
            );
        }
    }
    if diagnostics.has_errors() {
        return Err(diagnostic_failure(sources, diagnostics));
    }
    let mir = lower_hir(&hir);
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
