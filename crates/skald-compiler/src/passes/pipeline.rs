//! Target-independent MIR pass registration, execution, and accounting.

use crate::mir::{MirProgram, MirVerificationErrors};

use super::static_lifecycle;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct MirPipelineStatistics {
    verification_executions: u64,
    pass_executions: u64,
}

impl MirPipelineStatistics {
    pub(crate) const fn verification_executions(self) -> u64 {
        self.verification_executions
    }

    pub(crate) const fn pass_executions(self) -> u64 {
        self.pass_executions
    }
}

pub(crate) struct MeasuredMirPipeline {
    pub(crate) result: Result<MirProgram, MirVerificationErrors>,
    pub(crate) statistics: MirPipelineStatistics,
}

/// Runs the target-independent MIR pass pipeline.
///
/// No transformations are currently registered, but this explicit boundary
/// keeps correctness independent of a backend-owned implicit pipeline.
/// Verification runs here after MIR construction and backends verify again at
/// their trust boundary before target lowering.
pub fn run_mir_pipeline(program: MirProgram) -> Result<MirProgram, MirVerificationErrors> {
    run_mir_pipeline_measured(program).result
}

/// Runs the pipeline while retaining its already-known execution counts.
///
/// A future transformation must return its transformed program together with
/// pass-owned statistics to this coordinator. The pipeline, rather than the
/// pass or driver, then records the execution and publishes those values. A
/// pass must not format or emit reporting text itself.
pub(crate) fn run_mir_pipeline_measured(program: MirProgram) -> MeasuredMirPipeline {
    let statistics = MirPipelineStatistics {
        verification_executions: 1,
        pass_executions: 0,
    };
    let result = static_lifecycle::verify_synthesized_mir(&program).map(|()| program);
    MeasuredMirPipeline { result, statistics }
}

#[cfg(test)]
mod tests {
    use crate::test_support::lower_source_to_mir;

    use super::*;

    fn lowered_program() -> MirProgram {
        lower_source_to_mir("fn main() -> i64 { return 0; }")
    }

    #[test]
    fn empty_pipeline_preserves_valid_mir_and_reports_only_verification() {
        let mir = lowered_program();
        let expected = mir.clone();
        let measured = run_mir_pipeline_measured(mir);

        assert_eq!(measured.result.unwrap(), expected);
        assert_eq!(measured.statistics.verification_executions(), 1);
        assert_eq!(measured.statistics.pass_executions(), 0);
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
    fn rejected_mir_still_reports_the_verification_execution() {
        let mut mir = lowered_program();
        mir.definitions
            .get_mut_for_test(mir.entry_function)
            .unwrap()
            .body
            .blocks[0]
            .terminator = None;

        let measured = run_mir_pipeline_measured(mir);
        assert!(measured
            .result
            .unwrap_err()
            .to_string()
            .contains("block has no terminator"));
        assert_eq!(measured.statistics.verification_executions(), 1);
        assert_eq!(measured.statistics.pass_executions(), 0);
    }
}
