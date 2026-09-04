//! Verified ownership transfer and deterministic final-MIR pass execution.

mod error;
mod final_cfg;
mod inspection;
mod measurement;
mod model;
mod runner;
mod statistics;

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
pub(crate) use runner::{
    run_mir_pipeline_measured_inspected, run_mir_pipeline_with_transition_for_test,
};
pub(crate) use statistics::{MeasuredMirPipeline, MirPipelineStatistics};
