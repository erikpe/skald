use crate::mir::{rewrite::MirProgramRewriteResult, MirProgram};

use super::{
    error::MirPipelineError,
    inspection::{
        MirFinalPipelineCheckpoint, MirPipelineCheckpoint, MirPipelineCheckpointLabel,
        MirPipelineInspector, MirProofPipelineCheckpoint,
    },
    measurement::MirPassOccurrenceOutcome,
    model::{
        MirFinalPassCapability, MirFinalPassChange, MirFinalPassOutcome, MirPassFailure,
        MirProofPassCapability, MirProofPassChange, MirProofPassOutcome,
    },
    observation::{MirPassAttempt, MirPassOccurrenceRecorder},
    statistics::{MeasuredMirPipeline, MirPipelineStatistics},
    MirProofTransitionCapability, MirProofTransitionFailureKind, ProofNormalizationTransition,
};
use crate::passes::pipeline::{
    normalization::MirProofNormalizationStatistics,
    seal::{reseal_final_mir, transition_proof_mir, MirProofTransitionError},
    snapshot_analysis::{
        MirProofPassContext, MirProofSnapshotAnalysis, MirProofTransitionContext,
        MirSnapshotAnalysisCheckpoint, MirSnapshotAnalysisKind, MirSnapshotAnalysisPolicy,
        MirSnapshotAnalysisUsage,
    },
    verify_proof_mir, MirPassOccurrence, MirPassSchedule, VerifiedFinalMirProgram,
    VerifiedProofMirProgram,
};

struct MirPipelineExecution<'inspection> {
    inspector: Option<&'inspection mut dyn MirPipelineInspector>,
    statistics: MirPipelineStatistics,
    occurrence_recorder: MirPassOccurrenceRecorder,
}

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
        MirSnapshotAnalysisPolicy::Memoized,
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
    let mut execution = MirPipelineExecution::new(inspector, record_occurrences, schedule.len());
    let result = execution.run(program, schedule, transition, analysis_policy);
    execution.finish(result)
}

impl<'inspection> MirPipelineExecution<'inspection> {
    fn new(
        inspector: Option<&'inspection mut dyn MirPipelineInspector>,
        record_occurrences: bool,
        schedule_len: usize,
    ) -> Self {
        Self {
            inspector,
            statistics: MirPipelineStatistics::default(),
            occurrence_recorder: MirPassOccurrenceRecorder::new(record_occurrences, schedule_len),
        }
    }

    fn run(
        &mut self,
        program: MirProgram,
        schedule: &MirPassSchedule,
        transition: ProofNormalizationTransition,
        analysis_policy: MirSnapshotAnalysisPolicy,
    ) -> Result<VerifiedFinalMirProgram, MirPipelineError> {
        self.statistics.record_verification();
        let verified = verify_proof_mir(program).map_err(MirPipelineError::input_verification)?;
        self.inspect_proof(MirPipelineCheckpointLabel::ProofRichInput, &verified);

        let mut analyses = MirProofSnapshotAnalysis::new(analysis_policy);
        let verified = self.run_proof_rich_stage(verified, schedule, &mut analyses)?;
        let verified = self.run_proof_transition_stage(
            verified,
            schedule.proof_transition(),
            &mut analyses,
            transition,
        )?;
        let verified = self.run_final_stage(verified, schedule)?;

        self.inspect_final(MirPipelineCheckpointLabel::Final, &verified);
        Ok(verified)
    }

    fn run_proof_rich_stage(
        &mut self,
        mut verified: VerifiedProofMirProgram,
        schedule: &MirPassSchedule,
        analyses: &mut MirProofSnapshotAnalysis,
    ) -> Result<VerifiedProofMirProgram, MirPipelineError> {
        for occurrence in schedule.proof_rich() {
            verified = self.run_proof_rich_occurrence(verified, occurrence, analyses)?;
        }
        Ok(verified)
    }

    fn run_proof_rich_occurrence(
        &mut self,
        verified: VerifiedProofMirProgram,
        occurrence: MirPassOccurrence,
        analyses: &mut MirProofSnapshotAnalysis,
    ) -> Result<VerifiedProofMirProgram, MirPipelineError> {
        let attempt = self.start_pass(occurrence);
        let transform = occurrence
            .proof_transform()
            .expect("validated proof-rich occurrence must have a proof-rich callback");
        let checkpoint = analyses.checkpoint();
        let result = transform(MirProofPassContext::new(
            MirProofPassCapability::new(verified),
            analyses,
        ));
        let outcome = match result {
            Ok(outcome) => outcome,
            Err(failure) => {
                let analysis_usage = self.record_analysis_usage(analyses, checkpoint);
                return Err(self.record_pass_failure(
                    occurrence,
                    attempt,
                    Some((MirSnapshotAnalysisKind::LocalConstants, analysis_usage)),
                    failure,
                ));
            }
        };

        match outcome {
            MirProofPassOutcome::Unchanged {
                verified: unchanged,
                data,
            } => {
                let analysis_usage = self.record_analysis_usage(analyses, checkpoint);
                debug_assert_eq!(data.changed_callables(), 0);
                self.statistics.record_pass_data(occurrence, &data);
                self.occurrence_recorder.record_completed(
                    attempt,
                    MirPassOccurrenceOutcome::Unchanged,
                    data,
                    Default::default(),
                    0,
                    Some((MirSnapshotAnalysisKind::LocalConstants, analysis_usage)),
                );
                self.inspect_proof(after_proof_pass_label(occurrence), &unchanged);
                Ok(unchanged)
            }
            MirProofPassOutcome::Changed { change, data } => {
                analyses.reset();
                let analysis_usage = self.record_analysis_usage(analyses, checkpoint);
                self.statistics.record_pass_data(occurrence, &data);
                let (program, rewrite_changes) = match change {
                    MirProofPassChange::Rewrite(rewrite) => {
                        let rewrite_changes = self.statistics.record_rewrite(&rewrite);
                        let MirProgramRewriteResult { program, .. } = rewrite;
                        (program, rewrite_changes)
                    }
                };
                self.statistics.record_verification();
                let verified = match verify_proof_mir(program) {
                    Ok(verified) => {
                        self.occurrence_recorder.record_completed(
                            attempt,
                            MirPassOccurrenceOutcome::Changed,
                            data,
                            rewrite_changes,
                            1,
                            Some((MirSnapshotAnalysisKind::LocalConstants, analysis_usage)),
                        );
                        verified
                    }
                    Err(errors) => {
                        self.occurrence_recorder.record_completed(
                            attempt,
                            MirPassOccurrenceOutcome::Failed,
                            data,
                            rewrite_changes,
                            1,
                            Some((MirSnapshotAnalysisKind::LocalConstants, analysis_usage)),
                        );
                        return Err(MirPipelineError::output_verification(occurrence, errors));
                    }
                };
                self.inspect_proof(after_proof_pass_label(occurrence), &verified);
                Ok(verified)
            }
        }
    }

    fn run_proof_transition_stage(
        &mut self,
        verified: VerifiedProofMirProgram,
        occurrence: Option<MirPassOccurrence>,
        analyses: &mut MirProofSnapshotAnalysis,
        transition: ProofNormalizationTransition,
    ) -> Result<VerifiedFinalMirProgram, MirPipelineError> {
        let (verified, normalization) = match occurrence {
            Some(occurrence) => {
                self.run_selected_proof_transition(verified, occurrence, analyses, transition)?
            }
            None => self.run_implicit_proof_transition(verified, analyses, transition)?,
        };
        self.statistics
            .record_normalization_statistics(normalization);
        self.inspect_final(
            MirPipelineCheckpointLabel::AfterProofNormalization,
            &verified,
        );
        Ok(verified)
    }

    fn run_selected_proof_transition(
        &mut self,
        verified: VerifiedProofMirProgram,
        occurrence: MirPassOccurrence,
        analyses: &mut MirProofSnapshotAnalysis,
        transition: ProofNormalizationTransition,
    ) -> Result<(VerifiedFinalMirProgram, MirProofNormalizationStatistics), MirPipelineError> {
        let attempt = self.start_pass(occurrence);
        let transform = occurrence
            .transition_transform()
            .expect("validated transition occurrence must have a transition callback");
        let checkpoint = analyses.checkpoint();
        let result = transform(MirProofTransitionContext::new(
            MirProofTransitionCapability::with_transition(verified, transition),
            analyses,
        ));
        analyses.reset();
        let analysis_usage = self.record_analysis_usage(analyses, checkpoint);
        let outcome = match result {
            Ok(outcome) => outcome,
            Err(failure) => {
                let error = match failure.into_kind() {
                    MirProofTransitionFailureKind::Pass(failure) => {
                        pass_failure_error(occurrence, failure)
                    }
                    MirProofTransitionFailureKind::Boundary(error) => {
                        self.statistics.record_normalization_execution();
                        self.statistics.record_verification();
                        transition_boundary_error(Some(occurrence), error)
                    }
                };
                self.occurrence_recorder.record_failed(
                    attempt,
                    Some((MirSnapshotAnalysisKind::LocalConstants, analysis_usage)),
                );
                return Err(error);
            }
        };

        self.statistics.record_normalization_execution();
        self.statistics.record_verification();
        let (verified, normalization, data, changed) = outcome.into_parts();
        self.statistics.record_pass_data(occurrence, &data);
        self.occurrence_recorder.record_completed(
            attempt,
            if changed {
                MirPassOccurrenceOutcome::Changed
            } else {
                MirPassOccurrenceOutcome::Unchanged
            },
            data,
            Default::default(),
            1,
            Some((MirSnapshotAnalysisKind::LocalConstants, analysis_usage)),
        );
        self.inspect_final(after_transition_pass_label(occurrence), &verified);
        Ok((verified, normalization))
    }

    fn run_implicit_proof_transition(
        &mut self,
        verified: VerifiedProofMirProgram,
        analyses: &mut MirProofSnapshotAnalysis,
        transition: ProofNormalizationTransition,
    ) -> Result<(VerifiedFinalMirProgram, MirProofNormalizationStatistics), MirPipelineError> {
        let checkpoint = analyses.checkpoint();
        analyses.reset();
        self.record_analysis_usage(analyses, checkpoint);
        self.statistics.record_normalization_execution();
        self.statistics.record_verification();
        transition(verified, None).map_err(|error| transition_boundary_error(None, error))
    }

    fn run_final_stage(
        &mut self,
        mut verified: VerifiedFinalMirProgram,
        schedule: &MirPassSchedule,
    ) -> Result<VerifiedFinalMirProgram, MirPipelineError> {
        for occurrence in schedule.final_stage() {
            verified = self.run_final_occurrence(verified, occurrence)?;
        }
        Ok(verified)
    }

    fn run_final_occurrence(
        &mut self,
        verified: VerifiedFinalMirProgram,
        occurrence: MirPassOccurrence,
    ) -> Result<VerifiedFinalMirProgram, MirPipelineError> {
        let attempt = self.start_pass(occurrence);
        let transform = occurrence
            .final_transform()
            .expect("validated final-stage occurrence must have a final-stage callback");
        let outcome = match transform(MirFinalPassCapability::new(verified)) {
            Ok(outcome) => outcome,
            Err(failure) => {
                return Err(self.record_pass_failure(occurrence, attempt, None, failure));
            }
        };

        match outcome {
            MirFinalPassOutcome::Unchanged {
                verified: unchanged,
                data,
            } => {
                debug_assert_eq!(data.changed_callables(), 0);
                self.statistics.record_pass_data(occurrence, &data);
                self.occurrence_recorder.record_completed(
                    attempt,
                    MirPassOccurrenceOutcome::Unchanged,
                    data,
                    Default::default(),
                    0,
                    None,
                );
                self.inspect_final(after_final_pass_label(occurrence), &unchanged);
                Ok(unchanged)
            }
            MirFinalPassOutcome::Changed { change, data } => {
                self.statistics.record_pass_data(occurrence, &data);
                let (unverified, rewrite_changes) = match change {
                    MirFinalPassChange::DefinitionRetention(unverified) => {
                        (unverified, Default::default())
                    }
                    MirFinalPassChange::Rewrite(rewrite) => {
                        let changes = self
                            .statistics
                            .record_callable_rewrites(rewrite.callables());
                        (rewrite.into_unverified(), changes)
                    }
                };
                self.statistics.record_verification();
                let verified = match reseal_final_mir(unverified) {
                    Ok(verified) => {
                        self.occurrence_recorder.record_completed(
                            attempt,
                            MirPassOccurrenceOutcome::Changed,
                            data,
                            rewrite_changes,
                            1,
                            None,
                        );
                        verified
                    }
                    Err(errors) => {
                        self.occurrence_recorder.record_completed(
                            attempt,
                            MirPassOccurrenceOutcome::Failed,
                            data,
                            rewrite_changes,
                            1,
                            None,
                        );
                        return Err(MirPipelineError::output_verification(occurrence, errors));
                    }
                };
                self.inspect_final(after_final_pass_label(occurrence), &verified);
                Ok(verified)
            }
        }
    }

    fn start_pass(&mut self, occurrence: MirPassOccurrence) -> MirPassAttempt {
        self.statistics.record_pass_execution();
        self.occurrence_recorder.start(occurrence)
    }

    fn record_pass_failure(
        &mut self,
        occurrence: MirPassOccurrence,
        attempt: MirPassAttempt,
        analysis_usage: Option<(MirSnapshotAnalysisKind, MirSnapshotAnalysisUsage)>,
        failure: MirPassFailure,
    ) -> MirPipelineError {
        self.occurrence_recorder
            .record_failed(attempt, analysis_usage);
        pass_failure_error(occurrence, failure)
    }

    fn record_analysis_usage(
        &mut self,
        analyses: &MirProofSnapshotAnalysis,
        checkpoint: MirSnapshotAnalysisCheckpoint,
    ) -> MirSnapshotAnalysisUsage {
        let usage = analyses.usage_since(checkpoint);
        self.statistics
            .record_analysis_usage(MirSnapshotAnalysisKind::LocalConstants, usage);
        usage
    }

    fn inspect_proof(
        &mut self,
        label: MirPipelineCheckpointLabel,
        verified: &VerifiedProofMirProgram,
    ) {
        if let Some(inspector) = self.inspector.as_deref_mut() {
            inspector.inspect(MirPipelineCheckpoint::ProofRich(
                MirProofPipelineCheckpoint::new(label, verified),
            ));
        }
    }

    fn inspect_final(
        &mut self,
        label: MirPipelineCheckpointLabel,
        verified: &VerifiedFinalMirProgram,
    ) {
        if let Some(inspector) = self.inspector.as_deref_mut() {
            inspector.inspect(MirPipelineCheckpoint::Final(
                MirFinalPipelineCheckpoint::new(label, verified),
            ));
        }
    }

    fn finish(
        self,
        result: Result<VerifiedFinalMirProgram, MirPipelineError>,
    ) -> MeasuredMirPipeline {
        MeasuredMirPipeline::new(
            result,
            self.statistics,
            self.occurrence_recorder.into_records(),
        )
    }
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

fn pass_failure_error(occurrence: MirPassOccurrence, failure: MirPassFailure) -> MirPipelineError {
    match failure {
        MirPassFailure::Execution(error) => MirPipelineError::pass_execution(occurrence, error),
        MirPassFailure::Rewrite(error) => MirPipelineError::structural_rewrite(occurrence, error),
    }
}
