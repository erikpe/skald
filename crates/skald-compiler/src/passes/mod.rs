//! Explicit analyses, verification, and transformation pipelines over IR.
//!
//! Pass ordering belongs in named pipelines rather than being hidden inside
//! unrelated phase implementations.

pub mod static_lifecycle;

mod graph;
mod pipeline;
mod redundancy;
// Reachability is seal-bound but remains compiler-internal until its first
// retention and backend consumers land.
#[allow(dead_code, unused_imports)]
pub(crate) mod reachability;

// The exact resolver is a frozen compiler-internal experiment surface; no
// production adapter selects it yet.
pub use pipeline::{
    available_mir_passes, run_mir_pipeline, run_mir_pipeline_inspected, verify_final_mir,
    MirOptimizationProfile, MirPassDescriptor, MirPassIdentity, MirPassMeasurement,
    MirPassOccurrenceOutcome, MirPassOccurrenceRecord, MirPipelineCheckpoint,
    MirPipelineCheckpointLabel, MirPipelineError, MirPipelineFailureStage, MirPipelineInspector,
    VerifiedFinalMirProgram,
};
#[allow(unused_imports)]
pub(crate) use pipeline::{
    resolve_exact_mir_pass_schedule, resolve_mir_pass_schedule, MirPassSchedule,
    MirPassScheduleError,
};
pub(crate) use pipeline::{
    run_mir_pipeline_instrumented, run_mir_pipeline_measured, run_mir_pipeline_with_occurrences,
    MeasuredMirPipeline, MirPipelineStatistics,
};
pub use redundancy::{
    analyze_scalar_spill_provenance, ScalarSpillBlocker, ScalarSpillCallableObservation,
    ScalarSpillConsumer, ScalarSpillCount, ScalarSpillDepth, ScalarSpillProvenanceCounts,
    ScalarSpillProvenanceObservation, ScalarSpillUnlock,
};
