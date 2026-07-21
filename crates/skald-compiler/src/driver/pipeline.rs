//! Source-to-assembly orchestration over explicit compiler phase boundaries.

use std::path::Path;

use crate::{
    backend::{emit_assembly, BackendError, Target},
    diagnostics::{Diagnostic, Diagnostics},
    hir::{HirBlock, HirLocalInitializer, HirProgram, HirStatement},
    lexer::lex,
    mir::lower_hir,
    passes::run_mir_pipeline,
    resolve::resolve,
    source::SourceDatabase,
    syntax::parse,
    typeck::type_check,
};

const OBJECT_COPY_MIR_UNAVAILABLE: &str = "DRV001";

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
    if let Some(span) = first_runtime_copy_span(&hir) {
        diagnostics.push(
            Diagnostic::error(
                OBJECT_COPY_MIR_UNAVAILABLE,
                "object copy execution has not reached MIR yet",
            )
            .with_primary_label(span, "this OVS3 operation is available through typed HIR")
            .with_note("OVS4 adds target-independent MIR copy operations"),
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

fn first_runtime_copy_span(program: &HirProgram) -> Option<crate::source::Span> {
    program
        .definitions
        .iter()
        .find_map(|definition| first_copy_span(&definition.body))
        .or_else(|| {
            program.class_definitions.iter().find_map(|class| {
                std::iter::once(&class.initializer)
                    .chain(class.destructor.iter())
                    .chain(&class.methods)
                    .find_map(|definition| first_copy_span(&definition.body))
            })
        })
}

fn first_copy_span(block: &HirBlock) -> Option<crate::source::Span> {
    block
        .statements
        .iter()
        .find_map(|statement| match statement {
            HirStatement::Local(local)
                if matches!(local.initializer, HirLocalInitializer::Copy(_)) =>
            {
                Some(local.span)
            }
            HirStatement::CopyAssignment(assignment) => Some(assignment.span),
            HirStatement::Conditional(conditional) => conditional
                .arms
                .iter()
                .find_map(|arm| first_copy_span(&arm.body))
                .or_else(|| conditional.else_block.as_ref().and_then(first_copy_span)),
            HirStatement::Block(block) => first_copy_span(block),
            HirStatement::Local(_)
            | HirStatement::Return(_)
            | HirStatement::Call(_)
            | HirStatement::FieldAssignment(_)
            | HirStatement::FieldConstruction(_)
            | HirStatement::FieldCopyConstruction(_)
            | HirStatement::FieldCopyAssignment(_) => None,
        })
}

fn diagnostic_failure(sources: SourceDatabase, diagnostics: Diagnostics) -> CompilationError {
    CompilationError::Diagnostics(CompilationReport {
        sources,
        diagnostics,
    })
}
