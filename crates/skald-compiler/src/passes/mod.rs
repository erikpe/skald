//! Explicit analyses, verification, and transformation pipelines over IR.
//!
//! Pass ordering belongs in named pipelines rather than being hidden inside
//! unrelated phase implementations.

use crate::mir::{verify_mir, MirProgram, MirVerificationErrors};

/// Runs the target-independent MIR pass pipeline.
///
/// No transformations are currently enabled, but this explicit boundary keeps
/// correctness independent of a backend-owned implicit pipeline. Verification
/// runs here after MIR construction and backends verify again at their trust
/// boundary before target lowering.
pub fn run_mir_pipeline(program: MirProgram) -> Result<MirProgram, MirVerificationErrors> {
    verify_mir(&program)?;
    Ok(program)
}

#[cfg(test)]
mod tests {
    use crate::test_support::lower_source_to_mir;

    use super::*;

    fn lowered_program() -> MirProgram {
        lower_source_to_mir("fn main() -> i64 { return 0; }")
    }

    #[test]
    fn empty_pipeline_preserves_valid_mir() {
        let mir = lowered_program();
        let expected = mir.clone();

        assert_eq!(run_mir_pipeline(mir).unwrap(), expected);
    }

    #[test]
    fn pipeline_preserves_logical_path_and_cleanup_metadata() {
        let mir = lower_source_to_mir(
            "class Flag {
               truth: bool;
               init(truth: bool) { self.truth = truth; }
               fn read() -> bool { return self.truth; }
               destroy {}
             }
             fn make(truth: bool) -> shared Flag { return new Flag(truth); }
             fn evaluate(left: bool) -> bool {
               return left && make(true)->read();
             }
             fn main() -> i64 { return 0; }",
        );
        assert!(mir
            .definitions
            .iter()
            .any(|definition| !definition.body.path_conditions.is_empty()));
        let expected = mir.clone();

        assert_eq!(run_mir_pipeline(mir).unwrap(), expected);
    }

    #[test]
    fn pipeline_preserves_valid_multi_block_mir() {
        let mut mir = lowered_program();
        let function = mir
            .definitions
            .get_mut_for_test(mir.entry_function)
            .unwrap();
        let span = function.span;
        let second = crate::mir::BlockId::new(function.function, 1);
        function.body.blocks.push(crate::mir::MirBasicBlock {
            id: second,
            instructions: Vec::new(),
            terminator: Some(crate::mir::MirTerminator::Goto {
                target: second,
                span,
            }),
            span,
        });
        let expected = mir.clone();

        assert_eq!(run_mir_pipeline(mir).unwrap(), expected);
    }

    #[test]
    fn pipeline_preserves_pure_and_checked_primitive_casts_exactly() {
        let mir = lower_source_to_mir(
            "fn source() -> f64 { return 7.9; }
             fn main() -> i64 { return (i64) source() + (i64) (f64) 1u; }",
        );
        let expected = mir.clone();

        assert_eq!(run_mir_pipeline(mir).unwrap(), expected);
    }

    #[test]
    fn pipeline_rejects_invalid_mir_before_a_backend_sees_it() {
        let mut mir = lowered_program();
        mir.definitions
            .get_mut_for_test(mir.entry_function)
            .unwrap()
            .body
            .blocks[0]
            .terminator = None;

        let errors = run_mir_pipeline(mir).unwrap_err();
        assert!(errors.to_string().contains("block has no terminator"));
    }
}
