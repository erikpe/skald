use std::time::Instant;

use crate::mir::{rewrite::MirProgramRewriteResult, MirProgram};

use super::{
    error::MirPipelineError,
    inspection::{MirPipelineCheckpoint, MirPipelineCheckpointLabel, MirPipelineInspector},
    measurement::{MirPassOccurrenceOutcome, MirPassOccurrenceRecord},
    model::{
        MirFinalPassCapability, MirFinalPassChange, MirFinalPassOutcome, MirPassFailure,
        MirProofPassCapability, MirProofPassChange, MirProofPassOutcome,
    },
    statistics::{MeasuredMirPipeline, MirPipelineStatistics},
};
use crate::passes::pipeline::{
    seal::{finalize_proof_mir, reseal_final_mir},
    verify_proof_mir, MirPassOccurrence, MirPassSchedule,
};

pub(crate) fn run_mir_pipeline_measured(
    program: MirProgram,
    schedule: &MirPassSchedule,
) -> MeasuredMirPipeline {
    run_mir_pipeline_instrumented(program, schedule, false, None)
}

pub(crate) fn run_mir_pipeline_with_occurrences(
    program: MirProgram,
    schedule: &MirPassSchedule,
) -> MeasuredMirPipeline {
    run_mir_pipeline_instrumented(program, schedule, true, None)
}

#[cfg(test)]
pub(crate) fn run_mir_pipeline_measured_inspected(
    program: MirProgram,
    schedule: &MirPassSchedule,
    inspector: Option<&mut dyn MirPipelineInspector>,
) -> MeasuredMirPipeline {
    run_mir_pipeline_instrumented(program, schedule, false, inspector)
}

pub(crate) fn run_mir_pipeline_instrumented(
    program: MirProgram,
    schedule: &MirPassSchedule,
    record_occurrences: bool,
    inspector: Option<&mut dyn MirPipelineInspector>,
) -> MeasuredMirPipeline {
    let mut inspector = inspector;
    let mut statistics = MirPipelineStatistics::default();
    let mut records = if record_occurrences {
        Vec::with_capacity(schedule.len())
    } else {
        Vec::new()
    };
    statistics.record_verification();
    let mut verified = match verify_proof_mir(program) {
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

    for occurrence in schedule.proof_rich() {
        statistics.record_pass_execution();
        let started = record_occurrences.then(Instant::now);
        let transform = occurrence
            .proof_transform()
            .expect("validated proof-rich occurrence must have a proof-rich callback");
        let outcome = match transform(MirProofPassCapability::new(verified)) {
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
            MirProofPassOutcome::Unchanged {
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
            MirProofPassOutcome::Changed { change, data } => {
                statistics.record_pass_data(occurrence, &data);
                let (program, rewrite_changes) = match change {
                    MirProofPassChange::Rewrite(rewrite) => {
                        let rewrite_changes = statistics.record_rewrite(&rewrite);
                        let MirProgramRewriteResult { program, .. } = rewrite;
                        (program, rewrite_changes)
                    }
                    MirProofPassChange::DefinitionRetention(program) => {
                        (program, Default::default())
                    }
                };
                statistics.record_verification();
                verified = match verify_proof_mir(program) {
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

    statistics.record_verification();
    let mut verified = match finalize_proof_mir(verified) {
        Ok((verified, _normalization)) => verified,
        Err(errors) => {
            return MeasuredMirPipeline::new(
                Err(MirPipelineError::final_verification(errors)),
                statistics,
                records,
            );
        }
    };

    for occurrence in schedule.final_stage() {
        statistics.record_pass_execution();
        let started = record_occurrences.then(Instant::now);
        let transform = occurrence
            .final_transform()
            .expect("validated final-stage occurrence must have a final-stage callback");
        let outcome = match transform(MirFinalPassCapability::new(verified)) {
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
            MirFinalPassOutcome::Unchanged {
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
            MirFinalPassOutcome::Changed { change, data } => {
                statistics.record_pass_data(occurrence, &data);
                let MirFinalPassChange::DefinitionRetention(unverified) = change;
                let rewrite_changes = Default::default();
                statistics.record_verification();
                verified = match reseal_final_mir(unverified) {
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
    verified: &crate::passes::VerifiedProofMirProgram,
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
