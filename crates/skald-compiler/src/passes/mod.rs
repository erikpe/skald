//! Explicit analyses, verification, and transformation pipelines over IR.
//!
//! Pass ordering belongs in named pipelines rather than being hidden inside
//! unrelated phase implementations.

use crate::mir::{verify_mir, MirProgram, MirVerificationErrors};

/// Runs the target-independent MIR pass pipeline.
///
/// The first vertical slice has no transformations, but this explicit boundary
/// prevents correctness from depending on a backend-owned implicit pipeline.
/// Verification runs here after MIR construction and backends verify again at
/// their trust boundary before target lowering.
pub fn run_mir_pipeline(program: MirProgram) -> Result<MirProgram, MirVerificationErrors> {
    verify_mir(&program)?;
    Ok(program)
}

#[cfg(test)]
mod tests {
    use crate::{
        lexer::lex, mir::lower_hir, resolve::resolve, source::SourceDatabase, syntax::parse,
        typeck::type_check,
    };

    use super::*;

    fn lowered_program() -> MirProgram {
        let mut sources = SourceDatabase::new();
        let id = sources.add("passes.ska", "fn main() -> i64 { return 0; }");
        let source = sources.get(id).unwrap();
        let lexed = lex(source);
        let parsed = parse(source, &lexed.tokens);
        let resolved = resolve(&parsed.ast);
        let checked = type_check(&resolved.program);
        lower_hir(&checked.hir.unwrap())
    }

    #[test]
    fn first_slice_pipeline_preserves_valid_mir() {
        let mir = lowered_program();
        let expected = mir.clone();

        assert_eq!(run_mir_pipeline(mir).unwrap(), expected);
    }

    #[test]
    fn pipeline_rejects_invalid_mir_before_a_backend_sees_it() {
        let mut mir = lowered_program();
        mir.functions.entries_mut_for_test()[0].body.blocks[0].terminator = None;

        let errors = run_mir_pipeline(mir).unwrap_err();
        assert!(errors.to_string().contains("block has no terminator"));
    }
}
