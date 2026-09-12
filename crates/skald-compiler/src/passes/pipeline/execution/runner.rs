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
    MirProofTransitionCapability, MirProofTransitionFailureKind, ProofNormalizationTransition,
};
use crate::passes::pipeline::{
    seal::{reseal_final_mir, transition_proof_mir, MirProofTransitionError},
    snapshot_analysis::{
        MirProofPassContext, MirProofSnapshotAnalysis, MirProofTransitionContext,
        MirSnapshotAnalysisCheckpoint, MirSnapshotAnalysisKind, MirSnapshotAnalysisPolicy,
        MirSnapshotAnalysisUsage,
    },
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
        transition_proof_mir,
        MirSnapshotAnalysisPolicy::MeasureOnly,
    )
}

#[cfg(test)]
pub(in crate::passes::pipeline) fn run_mir_pipeline_with_transition_for_test(
    program: MirProgram,
    schedule: &MirPassSchedule,
    inspector: Option<&mut dyn MirPipelineInspector>,
    transition: ProofNormalizationTransition,
) -> MeasuredMirPipeline {
    run_mir_pipeline_with_transition(
        program,
        schedule,
        false,
        inspector,
        transition,
        MirSnapshotAnalysisPolicy::MeasureOnly,
    )
}

#[cfg(test)]
pub(in crate::passes::pipeline) fn run_mir_pipeline_with_transition_and_occurrences_for_test(
    program: MirProgram,
    schedule: &MirPassSchedule,
    inspector: Option<&mut dyn MirPipelineInspector>,
    transition: ProofNormalizationTransition,
) -> MeasuredMirPipeline {
    run_mir_pipeline_with_transition(
        program,
        schedule,
        true,
        inspector,
        transition,
        MirSnapshotAnalysisPolicy::MeasureOnly,
    )
}

#[cfg(test)]
pub(in crate::passes::pipeline) fn run_mir_pipeline_with_analysis_policy_for_test(
    program: MirProgram,
    schedule: &MirPassSchedule,
    inspector: Option<&mut dyn MirPipelineInspector>,
    policy: MirSnapshotAnalysisPolicy,
) -> MeasuredMirPipeline {
    run_mir_pipeline_with_transition(
        program,
        schedule,
        true,
        inspector,
        transition_proof_mir,
        policy,
    )
}

fn run_mir_pipeline_with_transition(
    program: MirProgram,
    schedule: &MirPassSchedule,
    record_occurrences: bool,
    inspector: Option<&mut dyn MirPipelineInspector>,
    transition: ProofNormalizationTransition,
    analysis_policy: MirSnapshotAnalysisPolicy,
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
    let mut analyses = MirProofSnapshotAnalysis::new(analysis_policy);

    for occurrence in schedule.proof_rich() {
        statistics.record_pass_execution();
        let started = record_occurrences.then(Instant::now);
        let transform = occurrence
            .proof_transform()
            .expect("validated proof-rich occurrence must have a proof-rich callback");
        let checkpoint = analyses.checkpoint();
        let result = transform(MirProofPassContext::new(
            MirProofPassCapability::new(verified),
            &mut analyses,
        ));
        let outcome = match result {
            Ok(outcome) => outcome,
            Err(MirPassFailure::Execution(error)) => {
                let analysis_usage = record_analysis_usage(&analyses, checkpoint, &mut statistics);
                record_failure(
                    &mut records,
                    occurrence,
                    started,
                    analysis_records(analysis_usage),
                );
                return MeasuredMirPipeline::new(
                    Err(MirPipelineError::pass_execution(occurrence, error)),
                    statistics,
                    records,
                );
            }
            Err(MirPassFailure::Rewrite(error)) => {
                let analysis_usage = record_analysis_usage(&analyses, checkpoint, &mut statistics);
                record_failure(
                    &mut records,
                    occurrence,
                    started,
                    analysis_records(analysis_usage),
                );
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
                let analysis_usage = record_analysis_usage(&analyses, checkpoint, &mut statistics);
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
                        analysis_records(analysis_usage),
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
                analyses.reset();
                let analysis_usage = record_analysis_usage(&analyses, checkpoint, &mut statistics);
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
                                analysis_records(analysis_usage),
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
                                analysis_records(analysis_usage),
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

    let (mut verified, normalization) = if let Some(occurrence) = schedule.proof_transition() {
        statistics.record_pass_execution();
        let started = record_occurrences.then(Instant::now);
        let transform = occurrence
            .transition_transform()
            .expect("validated transition occurrence must have a transition callback");
        let checkpoint = analyses.checkpoint();
        let result = transform(MirProofTransitionContext::new(
            MirProofTransitionCapability::with_transition(verified, transition),
            &mut analyses,
        ));
        analyses.reset();
        let analysis_usage = record_analysis_usage(&analyses, checkpoint, &mut statistics);
        let outcome = match result {
            Ok(outcome) => outcome,
            Err(failure) => {
                let error = match failure.into_kind() {
                    MirProofTransitionFailureKind::Pass(MirPassFailure::Execution(error)) => {
                        MirPipelineError::pass_execution(occurrence, error)
                    }
                    MirProofTransitionFailureKind::Pass(MirPassFailure::Rewrite(error)) => {
                        MirPipelineError::structural_rewrite(occurrence, error)
                    }
                    MirProofTransitionFailureKind::Boundary(error) => {
                        statistics.record_normalization_execution();
                        statistics.record_verification();
                        transition_boundary_error(Some(occurrence), error)
                    }
                };
                record_failure(
                    &mut records,
                    occurrence,
                    started,
                    analysis_records(analysis_usage),
                );
                return MeasuredMirPipeline::new(Err(error), statistics, records);
            }
        };

        statistics.record_normalization_execution();
        statistics.record_verification();
        let (verified, normalization, data, changed) = outcome.into_parts();
        statistics.record_pass_data(occurrence, &data);
        if let Some(started) = started {
            records.push(MirPassOccurrenceRecord::completed(
                occurrence,
                started.elapsed(),
                if changed {
                    MirPassOccurrenceOutcome::Changed
                } else {
                    MirPassOccurrenceOutcome::Unchanged
                },
                data,
                Default::default(),
                1,
                analysis_records(analysis_usage),
            ));
        }
        inspect_final_checkpoint(
            &mut inspector,
            after_transition_pass_label(occurrence),
            &verified,
        );
        (verified, normalization)
    } else {
        let checkpoint = analyses.checkpoint();
        analyses.reset();
        record_analysis_usage(&analyses, checkpoint, &mut statistics);
        statistics.record_normalization_execution();
        statistics.record_verification();
        match transition(verified, None) {
            Ok(result) => result,
            Err(error) => {
                return MeasuredMirPipeline::new(
                    Err(transition_boundary_error(None, error)),
                    statistics,
                    records,
                );
            }
        }
    };
    statistics.record_normalization_statistics(normalization);
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
                record_failure(&mut records, occurrence, started, Vec::new());
                return MeasuredMirPipeline::new(
                    Err(MirPipelineError::pass_execution(occurrence, error)),
                    statistics,
                    records,
                );
            }
            Err(MirPassFailure::Rewrite(error)) => {
                record_failure(&mut records, occurrence, started, Vec::new());
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
                        Vec::new(),
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
                let (unverified, rewrite_changes) = match change {
                    MirFinalPassChange::DefinitionRetention(unverified) => {
                        (unverified, Default::default())
                    }
                    MirFinalPassChange::Rewrite(rewrite) => {
                        let changes = statistics.record_callable_rewrites(rewrite.callables());
                        (rewrite.into_unverified(), changes)
                    }
                };
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
                                Vec::new(),
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
                                Vec::new(),
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

fn after_transition_pass_label(occurrence: MirPassOccurrence) -> MirPipelineCheckpointLabel {
    MirPipelineCheckpointLabel::AfterProofTransitionPass {
        position: occurrence.position(),
        pass_name: occurrence.name(),
        occurrence: occurrence.occurrence(),
    }
}

fn transition_boundary_error(
    occurrence: Option<MirPassOccurrence>,
    error: MirProofTransitionError,
) -> MirPipelineError {
    match (occurrence, error) {
        (Some(occurrence), MirProofTransitionError::OptionalPlanRewrite(error)) => {
            MirPipelineError::structural_rewrite(occurrence, error)
        }
        (Some(occurrence), MirProofTransitionError::OptionalPlanVerification(errors)) => {
            MirPipelineError::output_verification(occurrence, errors)
        }
        (None, MirProofTransitionError::OptionalPlanRewrite(error)) => {
            MirPipelineError::proof_normalization(crate::mir::MirVerificationErrors::program(
                format!("unexpected proof-transition rewrite: {error}"),
            ))
        }
        (None, MirProofTransitionError::OptionalPlanVerification(errors)) => {
            MirPipelineError::proof_normalization(errors)
        }
        (_, MirProofTransitionError::Normalization(errors)) => {
            MirPipelineError::proof_normalization(errors)
        }
        (Some(occurrence), MirProofTransitionError::FinalVerification(errors)) => {
            MirPipelineError::output_verification(occurrence, errors)
        }
        (None, MirProofTransitionError::FinalVerification(errors)) => {
            MirPipelineError::proof_normalization(errors)
        }
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
    analysis_usage: Vec<(MirSnapshotAnalysisKind, MirSnapshotAnalysisUsage)>,
) {
    if let Some(started) = started {
        records.push(MirPassOccurrenceRecord::failed(
            occurrence,
            started.elapsed(),
            analysis_usage,
        ));
    }
}

fn record_analysis_usage(
    analyses: &MirProofSnapshotAnalysis,
    checkpoint: MirSnapshotAnalysisCheckpoint,
    statistics: &mut MirPipelineStatistics,
) -> MirSnapshotAnalysisUsage {
    let usage = analyses.usage_since(checkpoint);
    statistics.record_analysis_usage(MirSnapshotAnalysisKind::LocalConstants, usage);
    usage
}

fn analysis_records(
    usage: MirSnapshotAnalysisUsage,
) -> Vec<(MirSnapshotAnalysisKind, MirSnapshotAnalysisUsage)> {
    if usage.is_empty() {
        Vec::new()
    } else {
        vec![(MirSnapshotAnalysisKind::LocalConstants, usage)]
    }
}
