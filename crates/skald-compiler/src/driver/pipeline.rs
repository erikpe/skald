//! Source-to-assembly orchestration over explicit compiler phase boundaries.

use std::path::Path;

use crate::{
    backend::{emit_assembly, BackendError, Target},
    diagnostics::Diagnostics,
    lexer::lex,
    mir::lower_hir,
    passes::run_mir_pipeline,
    resolve::resolve,
    source::SourceDatabase,
    syntax::parse,
    typeck::type_check,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::render_diagnostics;

    #[test]
    fn composes_the_complete_frontend_and_backend_pipeline() {
        let artifact = compile_source_to_assembly(
            "complete.ska",
            "fn double(x: i64) -> i64 { return x * 2; }\n\
             fn main() -> i64 { return double(21); }",
            Target::X86_64SysV,
        )
        .unwrap();

        assert!(artifact.report.diagnostics.is_empty());
        assert!(artifact.assembly.contains("call ska_fn_0"));
        assert!(artifact.assembly.contains(".globl main"));
    }

    #[test]
    fn stops_before_semantic_phases_after_a_source_error() {
        let CompilationError::Diagnostics(report) = compile_source_to_assembly(
            "broken.ska",
            "fn main() -> i64 { return @; }",
            Target::X86_64SysV,
        )
        .unwrap_err() else {
            panic!("expected source diagnostics");
        };

        let rendered = render_diagnostics(&report.sources, &report.diagnostics);
        assert!(rendered.contains("error[LEX001]: unexpected character `@`"));
        assert!(!rendered.contains("PAR"));
    }

    #[test]
    fn malformed_first_slice_sources_never_panic() {
        let valid = "fn main() -> i64 { var value: i64 = 1; return value + 2; }";
        let mut malformed: Vec<&str> = valid
            .char_indices()
            .map(|(offset, _)| &valid[..offset])
            .collect();
        malformed.extend([
            "@",
            "fn",
            "fn main(",
            "fn main() -> i64 { return ; }",
            "fn main() -> i64 { var x: i64 = ; return 0; }",
            "fn main() -> i64 { (((((((; }",
            "fn main() -> i64 { return 12abc; }",
            "fn main() -> i64 { if 1 { return 0; } }",
            "fn main() -> bool { return 0; }",
            "fn main() -> i64 { return unknown(1, 2); }",
        ]);

        for (index, source) in malformed.into_iter().enumerate() {
            let result = std::panic::catch_unwind(|| {
                compile_source_to_assembly(
                    format!("malformed-{index}.ska"),
                    source,
                    Target::X86_64SysV,
                )
            });
            assert!(
                result.is_ok(),
                "compiler panicked for malformed input {source:?}"
            );
            assert!(
                matches!(result.unwrap(), Err(CompilationError::Diagnostics(_))),
                "malformed input did not produce source diagnostics: {source:?}"
            );
        }
    }
}
