//! Verified ownership transfer and deterministic final-MIR pass execution.

mod error;
mod final_cfg;
mod final_storage_cleanup;
mod inspection;
mod measurement;
mod model;
mod observation;
mod runner;
mod statistics;
mod transition;

pub(in crate::passes::pipeline) use super::snapshot_analysis::{
    MirProofPassContext, MirProofTransitionContext,
};
pub use error::{MirPipelineError, MirPipelineFailureStage};
#[cfg(test)]
pub(in crate::passes::pipeline) use final_storage_cleanup::test_support::{
    append_complete_dead_activation, append_declaration_only_dead_activation,
    append_materially_used_activation, SINGLE_DEAD_ACTIVATION_SOURCE,
};
pub(in crate::passes::pipeline) use final_storage_cleanup::MirFinalStorageCleanupPlan;
pub use inspection::{
    MirFinalPipelineCheckpoint, MirPipelineCheckpoint, MirPipelineCheckpointLabel,
    MirPipelineInspector, MirProofPipelineCheckpoint,
};
pub use measurement::{MirPassMeasurement, MirPassOccurrenceOutcome, MirPassOccurrenceRecord};
pub(in crate::passes::pipeline) use model::{
    MirFinalPassCapability, MirFinalPassOutcome, MirFinalPassTransform, MirPassData,
    MirPassFailure, MirProofChangedProgram, MirProofPassCapability, MirProofPassOutcome,
    MirProofPassTransform,
};
pub(crate) use runner::{
    run_mir_pipeline_instrumented, run_mir_pipeline_measured, run_mir_pipeline_with_occurrences,
};
#[cfg(test)]
pub(in crate::passes::pipeline) use runner::{
    run_mir_pipeline_measured_inspected, run_mir_pipeline_with_analysis_policy_for_test,
    run_mir_pipeline_with_transition_and_occurrences_for_test,
    run_mir_pipeline_with_transition_for_test,
};
pub(crate) use statistics::{MeasuredMirPipeline, MirPipelineStatistics};
pub(in crate::passes::pipeline) use transition::{
    MirProofTransitionCapability, MirProofTransitionFailure, MirProofTransitionFailureKind,
    MirProofTransitionOutcome, MirProofTransitionTransform, ProofNormalizationTransition,
};
