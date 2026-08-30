//! Verified ownership transfer and deterministic final-MIR pass execution.

mod error;
mod inspection;
mod measurement;
mod model;
mod runner;
mod statistics;

pub use error::{MirPipelineError, MirPipelineFailureStage};
pub use inspection::{MirPipelineCheckpoint, MirPipelineCheckpointLabel, MirPipelineInspector};
pub use measurement::{MirPassMeasurement, MirPassOccurrenceOutcome, MirPassOccurrenceRecord};
pub(in crate::passes::pipeline) use model::{
    MirPassCapability, MirPassData, MirPassFailure, MirPassOutcome, MirPassTransform,
};
pub(crate) use runner::{
    run_mir_pipeline_measured, run_mir_pipeline_measured_inspected,
    run_mir_pipeline_with_occurrences,
};
pub(crate) use statistics::{MeasuredMirPipeline, MirPipelineStatistics};
