//! Explicit analyses, verification, and transformation pipelines over IR.
//!
//! Pass ordering belongs in named pipelines rather than being hidden inside
//! unrelated phase implementations.

pub mod static_lifecycle;

mod graph;
mod pipeline;

#[cfg(test)]
pub(crate) use pipeline::run_transforming_mir_pipeline;
#[allow(unused_imports)]
pub(crate) use pipeline::{
    registered_mir_pass_names, resolve_exact_mir_pass_schedule, resolve_mir_pass_schedule,
    MirPassIdentity, MirPassOccurrence, MirPassSchedule, MirPassScheduleError,
};
pub use pipeline::{
    run_mir_pipeline, verify_final_mir, MirOptimizationProfile, VerifiedFinalMirProgram,
};
pub(crate) use pipeline::{run_mir_pipeline_measured, MeasuredMirPipeline, MirPipelineStatistics};
