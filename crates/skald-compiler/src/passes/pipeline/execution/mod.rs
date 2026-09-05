//! Verified ownership transfer and deterministic final-MIR pass execution.

mod error;
mod final_cfg;
mod inspection;
mod measurement;
mod model;
mod runner;
mod statistics;
mod transition;

pub use error::{MirPipelineError, MirPipelineFailureStage};
pub use inspection::{
    MirFinalPipelineCheckpoint, MirPipelineCheckpoint, MirPipelineCheckpointLabel,
    MirPipelineInspector, MirProofPipelineCheckpoint,
};
pub use measurement::{MirPassMeasurement, MirPassOccurrenceOutcome, MirPassOccurrenceRecord};
pub(in crate::passes::pipeline) use model::{
    MirFinalPassCapability, MirFinalPassOutcome, MirFinalPassTransform, MirPassData,
    MirPassFailure, MirProofPassCapability, MirProofPassOutcome, MirProofPassTransform,
};
pub(crate) use runner::{
    run_mir_pipeline_instrumented, run_mir_pipeline_measured, run_mir_pipeline_with_occurrences,
};
#[cfg(test)]
pub(in crate::passes::pipeline) use runner::{
    run_mir_pipeline_measured_inspected, run_mir_pipeline_with_transition_and_occurrences_for_test,
    run_mir_pipeline_with_transition_for_test,
};
pub(crate) use statistics::{MeasuredMirPipeline, MirPipelineStatistics};
#[allow(unused_imports)]
pub(in crate::passes::pipeline) use transition::{
    MirProofTransitionCapability, MirProofTransitionFailure, MirProofTransitionFailureKind,
    MirProofTransitionOutcome, MirProofTransitionTransform, ProofNormalizationTransition,
};
