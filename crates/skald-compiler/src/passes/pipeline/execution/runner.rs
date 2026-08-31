use std::time::Instant;

use crate::mir::{rewrite::MirProgramRewriteResult, MirProgram};

use super::{
    error::MirPipelineError,
    inspection::{MirPipelineCheckpoint, MirPipelineCheckpointLabel, MirPipelineInspector},
    measurement::{MirPassOccurrenceOutcome, MirPassOccurrenceRecord},
    model::{MirPassCapability, MirPassChange, MirPassFailure, MirPassOutcome},
    statistics::{MeasuredMirPipeline, MirPipelineStatistics},
};
use crate::passes::pipeline::{verify_final_mir, MirPassOccurrence, MirPassSchedule};

pub(crate) fn run_mir_pipeline_measured(
    program: MirProgram,
    schedule: &MirPassSchedule,
) -> MeasuredMirPipeline {
    run(program, schedule, false, None)
}

pub(crate) fn run_mir_pipeline_with_occurrences(
    program: MirProgram,
    schedule: &MirPassSchedule,
) -> MeasuredMirPipeline {
    run(program, schedule, true, None)
}

pub(crate) fn run_mir_pipeline_measured_inspected(
    program: MirProgram,
    schedule: &MirPassSchedule,
    inspector: Option<&mut dyn MirPipelineInspector>,
) -> MeasuredMirPipeline {
    run(program, schedule, false, inspector)
}

fn run(
    program: MirProgram,
    schedule: &MirPassSchedule,
    record_occurrences: bool,
    mut inspector: Option<&mut dyn MirPipelineInspector>,
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
    inspect_checkpoint(&mut inspector, MirPipelineCheckpointLabel::Input, &verified);

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
                inspect_checkpoint(&mut inspector, after_label(occurrence), &verified);
            }
            MirPassOutcome::Changed { change, data } => {
                statistics.record_pass_data(occurrence, &data);
                let (program, rewrite_changes) = match change {
                    MirPassChange::Rewrite(rewrite) => {
                        let rewrite_changes = statistics.record_rewrite(&rewrite);
                        let MirProgramRewriteResult { program, .. } = rewrite;
                        (program, rewrite_changes)
                    }
                    MirPassChange::DefinitionRetention(program) => (program, Default::default()),
                };
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
                inspect_checkpoint(&mut inspector, after_label(occurrence), &verified);
            }
        }
    }

    inspect_checkpoint(&mut inspector, MirPipelineCheckpointLabel::Final, &verified);

    MeasuredMirPipeline::new(Ok(verified), statistics, records)
}

fn after_label(occurrence: MirPassOccurrence) -> MirPipelineCheckpointLabel {
    MirPipelineCheckpointLabel::After {
        position: occurrence.position(),
        pass_name: occurrence.name(),
        occurrence: occurrence.occurrence(),
    }
}

fn inspect_checkpoint(
    inspector: &mut Option<&mut dyn MirPipelineInspector>,
    label: MirPipelineCheckpointLabel,
    verified: &crate::passes::VerifiedFinalMirProgram,
) {
    if let Some(inspector) = inspector.as_deref_mut() {
        inspector.inspect(MirPipelineCheckpoint::new(label, verified));
    }
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
