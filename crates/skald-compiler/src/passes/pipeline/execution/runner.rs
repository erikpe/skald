use std::time::Instant;

use crate::mir::{rewrite::MirProgramRewriteResult, MirProgram};

use super::{
    error::MirPipelineError,
    inspection::{
        MirFinalPipelineCheckpoint, MirPipelineCheckpoint, MirPipelineCheckpointLabel,
        MirPipelineInspector, MirProofPipelineCheckpoint,
    },
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
    run_mir_pipeline_with_transition(
        program,
        schedule,
        record_occurrences,
        inspector,
        finalize_proof_mir,
    )
}

#[cfg(test)]
pub(crate) fn run_mir_pipeline_with_transition_for_test(
    program: MirProgram,
    schedule: &MirPassSchedule,
    inspector: Option<&mut dyn MirPipelineInspector>,
    transition: ProofNormalizationTransition,
) -> MeasuredMirPipeline {
    run_mir_pipeline_with_transition(program, schedule, false, inspector, transition)
}

pub(crate) type ProofNormalizationTransition = fn(
    crate::passes::VerifiedProofMirProgram,
) -> Result<
    (
        crate::passes::VerifiedFinalMirProgram,
        crate::passes::pipeline::normalization::MirProofNormalizationStatistics,
    ),
    crate::mir::MirVerificationErrors,
>;

fn run_mir_pipeline_with_transition(
    program: MirProgram,
    schedule: &MirPassSchedule,
    record_occurrences: bool,
    inspector: Option<&mut dyn MirPipelineInspector>,
    transition: ProofNormalizationTransition,
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
    inspect_proof_checkpoint(
        &mut inspector,
        MirPipelineCheckpointLabel::ProofRichInput,
        &verified,
    );

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
                inspect_proof_checkpoint(
                    &mut inspector,
                    after_proof_pass_label(occurrence),
                    &verified,
                );
            }
            MirProofPassOutcome::Changed { change, data } => {
                statistics.record_pass_data(occurrence, &data);
                let (program, rewrite_changes) = match change {
                    MirProofPassChange::Rewrite(rewrite) => {
                        let rewrite_changes = statistics.record_rewrite(&rewrite);
                        let MirProgramRewriteResult { program, .. } = rewrite;
                        (program, rewrite_changes)
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
                inspect_proof_checkpoint(
                    &mut inspector,
                    after_proof_pass_label(occurrence),
                    &verified,
                );
            }
        }
    }

    statistics.record_normalization_execution();
    statistics.record_verification();
    let mut verified = match transition(verified) {
        Ok((verified, normalization)) => {
            statistics.record_normalization_statistics(normalization);
            verified
        }
        Err(errors) => {
            return MeasuredMirPipeline::new(
                Err(MirPipelineError::proof_normalization(errors)),
                statistics,
                records,
            );
        }
    };
    inspect_final_checkpoint(
        &mut inspector,
        MirPipelineCheckpointLabel::AfterProofNormalization,
        &verified,
    );

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
                inspect_final_checkpoint(
                    &mut inspector,
                    after_final_pass_label(occurrence),
                    &verified,
                );
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
                inspect_final_checkpoint(
                    &mut inspector,
                    after_final_pass_label(occurrence),
                    &verified,
                );
            }
        }
    }

    inspect_final_checkpoint(&mut inspector, MirPipelineCheckpointLabel::Final, &verified);

    MeasuredMirPipeline::new(Ok(verified), statistics, records)
}

fn after_proof_pass_label(occurrence: MirPassOccurrence) -> MirPipelineCheckpointLabel {
    MirPipelineCheckpointLabel::AfterProofRichPass {
        position: occurrence.position(),
        pass_name: occurrence.name(),
        occurrence: occurrence.occurrence(),
    }
}

fn after_final_pass_label(occurrence: MirPassOccurrence) -> MirPipelineCheckpointLabel {
    MirPipelineCheckpointLabel::AfterFinalPass {
        position: occurrence.position(),
        pass_name: occurrence.name(),
        occurrence: occurrence.occurrence(),
    }
}

fn inspect_proof_checkpoint(
    inspector: &mut Option<&mut dyn MirPipelineInspector>,
    label: MirPipelineCheckpointLabel,
    verified: &crate::passes::VerifiedProofMirProgram,
) {
    if let Some(inspector) = inspector.as_deref_mut() {
        inspector.inspect(MirPipelineCheckpoint::ProofRich(
            MirProofPipelineCheckpoint::new(label, verified),
        ));
    }
}

fn inspect_final_checkpoint(
    inspector: &mut Option<&mut dyn MirPipelineInspector>,
    label: MirPipelineCheckpointLabel,
    verified: &crate::passes::VerifiedFinalMirProgram,
) {
    if let Some(inspector) = inspector.as_deref_mut() {
        inspector.inspect(MirPipelineCheckpoint::Final(
            MirFinalPipelineCheckpoint::new(label, verified),
        ));
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
