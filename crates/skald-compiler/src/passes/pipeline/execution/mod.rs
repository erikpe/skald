//! Verified ownership transfer and deterministic final-MIR pass execution.

mod error;
mod model;
mod runner;
mod statistics;

pub use error::{MirPipelineError, MirPipelineFailureStage};
pub(in crate::passes::pipeline) use model::MirPassTransform;
#[cfg(test)]
pub(in crate::passes::pipeline) use model::{
    MirPassCapability, MirPassData, MirPassFailure, MirPassOutcome,
};
pub(crate) use runner::run_mir_pipeline_measured;
pub(crate) use statistics::{MeasuredMirPipeline, MirPipelineStatistics};
