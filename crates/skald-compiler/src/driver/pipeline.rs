//! Source-to-assembly orchestration over explicit compiler phase boundaries.

use std::path::Path;

use crate::{
    backend::{emit_assembly, BackendError, Target},
    diagnostics::{Diagnostic, Diagnostics},
    lexer::lex,
    mir::lower_hir,
    passes::run_mir_pipeline,
    resolve::resolve,
    source::SourceDatabase,
    syntax::parse,
    typeck::type_check,
};

pub const ALIAS_MIR_NOT_IMPLEMENTED: &str = "DRV001";

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
    Diagnostics(CompilationReport),
    MirVerification(crate::mir::MirVerificationErrors),
    Backend(BackendError),
}

/// Runs the complete target-independent pipeline and selected backend for one
/// source file. Source I/O and artifact publication remain driver concerns.
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
    if diagnostics.has_errors() {
        return Err(diagnostic_failure(sources, diagnostics));
    }

    let checked = type_check(&resolved.program);
    diagnostics.append(checked.diagnostics);
    if diagnostics.has_errors() {
        return Err(diagnostic_failure(sources, diagnostics));
    }
    let hir = checked
        .hir
        .expect("type checking without errors must produce typed HIR");
    if let Some(parameter) = hir.first_alias_parameter() {
        diagnostics.push(
            Diagnostic::error(
                ALIAS_MIR_NOT_IMPLEMENTED,
                "alias parameters are not available in MIR yet",
            )
            .with_primary_label(
                parameter.span,
                "typed alias signature reaches the AL3 boundary",
            )
            .with_note("AL4 adds MIR parameter modes and place arguments"),
        );
        return Err(diagnostic_failure(sources, diagnostics));
    }
    let mir = run_mir_pipeline(lower_hir(&hir)).map_err(CompilationError::MirVerification)?;
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
