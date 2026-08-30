//! Explicit analyses, verification, and transformation pipelines over IR.
//!
//! Pass ordering belongs in named pipelines rather than being hidden inside
//! unrelated phase implementations.

pub mod static_lifecycle;

mod graph;
mod pipeline;

#[allow(unused_imports)]
pub(crate) use pipeline::{
    registered_mir_pass_names, resolve_exact_mir_pass_schedule, resolve_mir_pass_schedule,
    MirPassOccurrence, MirPassSchedule, MirPassScheduleError,
};
pub use pipeline::{
    run_mir_pipeline, verify_final_mir, MirOptimizationProfile, MirPassIdentity,
    MirPassMeasurement, MirPassOccurrenceOutcome, MirPassOccurrenceRecord, MirPipelineError,
    MirPipelineFailureStage, VerifiedFinalMirProgram,
};
pub(crate) use pipeline::{
    run_mir_pipeline_measured, run_mir_pipeline_with_occurrences, MeasuredMirPipeline,
    MirPipelineStatistics,
};
