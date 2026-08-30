use crate::mir::{rewrite::MirProgramRewriteResult, MirProgram};

use super::{
    error::MirPipelineError,
    model::{MirPassCapability, MirPassFailure, MirPassOutcome},
    statistics::{MeasuredMirPipeline, MirPipelineStatistics},
};
use crate::passes::pipeline::{verify_final_mir, MirPassSchedule};

pub(crate) fn run_mir_pipeline_measured(
    program: MirProgram,
    schedule: &MirPassSchedule,
) -> MeasuredMirPipeline {
    let mut statistics = MirPipelineStatistics::default();
    statistics.record_verification();
    let mut verified = match verify_final_mir(program) {
        Ok(verified) => verified,
        Err(errors) => {
            return MeasuredMirPipeline {
                result: Err(MirPipelineError::input_verification(errors)),
                statistics,
            };
        }
    };

    for occurrence in schedule.iter() {
        statistics.record_pass_execution();
        let outcome = match (occurrence.transform())(MirPassCapability::new(verified)) {
            Ok(outcome) => outcome,
            Err(MirPassFailure::Execution(error)) => {
                return MeasuredMirPipeline {
                    result: Err(MirPipelineError::pass_execution(occurrence, error)),
                    statistics,
                };
            }
            Err(MirPassFailure::Rewrite(error)) => {
                return MeasuredMirPipeline {
                    result: Err(MirPipelineError::structural_rewrite(occurrence, error)),
                    statistics,
                };
            }
        };

        match outcome {
            MirPassOutcome::Unchanged {
                verified: unchanged,
                data,
            } => {
                debug_assert_eq!(data.changed_callables(), 0);
                verified = unchanged;
            }
            MirPassOutcome::Changed { rewrite, data } => {
                statistics.record_rewrite(&rewrite, data);
                let MirProgramRewriteResult { program, .. } = rewrite;
                statistics.record_verification();
                verified = match verify_final_mir(program) {
                    Ok(verified) => verified,
                    Err(errors) => {
                        return MeasuredMirPipeline {
                            result: Err(MirPipelineError::output_verification(occurrence, errors)),
                            statistics,
                        };
                    }
                };
            }
        }
    }

    MeasuredMirPipeline {
        result: Ok(verified),
        statistics,
    }
}
