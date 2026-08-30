use std::time::Instant;

use crate::mir::{rewrite::MirProgramRewriteResult, MirProgram};

use super::{
    error::MirPipelineError,
    measurement::{MirPassOccurrenceOutcome, MirPassOccurrenceRecord},
    model::{MirPassCapability, MirPassFailure, MirPassOutcome},
    statistics::{MeasuredMirPipeline, MirPipelineStatistics},
};
use crate::passes::pipeline::{verify_final_mir, MirPassOccurrence, MirPassSchedule};

pub(crate) fn run_mir_pipeline_measured(
    program: MirProgram,
    schedule: &MirPassSchedule,
) -> MeasuredMirPipeline {
    run(program, schedule, false)
}

pub(crate) fn run_mir_pipeline_with_occurrences(
    program: MirProgram,
    schedule: &MirPassSchedule,
) -> MeasuredMirPipeline {
    run(program, schedule, true)
}

fn run(
    program: MirProgram,
    schedule: &MirPassSchedule,
    record_occurrences: bool,
) -> MeasuredMirPipeline {
    let mut statistics = MirPipelineStatistics::default();
    let mut records = if record_occurrences {
        Vec::with_capacity(schedule.len())
    } else {
        Vec::new()
    };
    statistics.record_verification();
    let mut verified = match verify_final_mir(program) {
        Ok(verified) => verified,
        Err(errors) => {
            return MeasuredMirPipeline::new(
                Err(MirPipelineError::input_verification(errors)),
                statistics,
                records,
            );
        }
    };

    for occurrence in schedule.iter() {
        statistics.record_pass_execution();
        let started = record_occurrences.then(Instant::now);
        let outcome = match (occurrence.transform())(MirPassCapability::new(verified)) {
            Ok(outcome) => outcome,
            Err(MirPassFailure::Execution(error)) => {
                record_failure(&mut records, occurrence, started);
                return MeasuredMirPipeline::new(
                    Err(MirPipelineError::pass_execution(occurrence, error)),
                    statistics,
                    records,
                );
            }
            Err(MirPassFailure::Rewrite(error)) => {
                record_failure(&mut records, occurrence, started);
                return MeasuredMirPipeline::new(
                    Err(MirPipelineError::structural_rewrite(occurrence, error)),
                    statistics,
                    records,
                );
            }
        };

        match outcome {
            MirPassOutcome::Unchanged {
                verified: unchanged,
                data,
            } => {
                debug_assert_eq!(data.changed_callables(), 0);
                statistics.record_pass_data(occurrence, &data);
                if let Some(started) = started {
                    records.push(MirPassOccurrenceRecord::completed(
                        occurrence,
                        started.elapsed(),
                        MirPassOccurrenceOutcome::Unchanged,
                        data,
                        Default::default(),
                        0,
                    ));
                }
                verified = unchanged;
            }
            MirPassOutcome::Changed { rewrite, data } => {
                statistics.record_pass_data(occurrence, &data);
                let rewrite_changes = statistics.record_rewrite(&rewrite);
                let MirProgramRewriteResult { program, .. } = rewrite;
                statistics.record_verification();
                verified = match verify_final_mir(program) {
                    Ok(verified) => {
                        if let Some(started) = started {
                            records.push(MirPassOccurrenceRecord::completed(
                                occurrence,
                                started.elapsed(),
                                MirPassOccurrenceOutcome::Changed,
                                data,
                                rewrite_changes,
                                1,
                            ));
                        }
                        verified
                    }
                    Err(errors) => {
                        if let Some(started) = started {
                            records.push(MirPassOccurrenceRecord::completed(
                                occurrence,
                                started.elapsed(),
                                MirPassOccurrenceOutcome::Failed,
                                data,
                                rewrite_changes,
                                1,
                            ));
                        }
                        return MeasuredMirPipeline::new(
                            Err(MirPipelineError::output_verification(occurrence, errors)),
                            statistics,
                            records,
                        );
                    }
                };
            }
        }
    }

    MeasuredMirPipeline::new(Ok(verified), statistics, records)
}

fn record_failure(
    records: &mut Vec<MirPassOccurrenceRecord>,
    occurrence: MirPassOccurrence,
    started: Option<Instant>,
) {
    if let Some(started) = started {
        records.push(MirPassOccurrenceRecord::failed(
            occurrence,
            started.elapsed(),
        ));
    }
}
