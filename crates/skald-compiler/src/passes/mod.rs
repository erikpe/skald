//! Explicit analyses, verification, and transformation pipelines over IR.
//!
//! Pass ordering belongs in named pipelines rather than being hidden inside
//! unrelated phase implementations.

pub mod static_lifecycle;

mod graph;
mod pipeline;

// The exact resolver is a frozen compiler-internal experiment surface; no
// production adapter selects it yet.
#[allow(unused_imports)]
pub(crate) use pipeline::{
    resolve_exact_mir_pass_schedule, resolve_mir_pass_schedule, MirPassSchedule,
    MirPassScheduleError,
};
pub use pipeline::{
    run_mir_pipeline, run_mir_pipeline_inspected, verify_final_mir, MirOptimizationProfile,
    MirPassIdentity, MirPassMeasurement, MirPassOccurrenceOutcome, MirPassOccurrenceRecord,
    MirPipelineCheckpoint, MirPipelineCheckpointLabel, MirPipelineError, MirPipelineFailureStage,
    MirPipelineInspector, VerifiedFinalMirProgram,
};
pub(crate) use pipeline::{
    run_mir_pipeline_measured, run_mir_pipeline_with_occurrences, MeasuredMirPipeline,
    MirPipelineStatistics,
};
