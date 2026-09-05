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

pub use pipeline::{
    available_mir_passes, run_mir_pipeline, run_mir_pipeline_inspected, verify_final_mir,
    MirFinalPipelineCheckpoint, MirOptimizationProfile, MirPassDescriptor, MirPassIdentity,
    MirPassMeasurement, MirPassOccurrenceOutcome, MirPassOccurrenceRecord, MirPassStage,
    MirPipelineCheckpoint, MirPipelineCheckpointLabel, MirPipelineError, MirPipelineFailureStage,
    MirPipelineInspector, MirProofPipelineCheckpoint, VerifiedFinalMirProgram,
    VerifiedProofMirProgram,
};
#[cfg(test)]
pub(crate) use pipeline::{resolve_exact_mir_pass_schedule, verify_proof_mir};
pub(crate) use pipeline::{resolve_mir_pass_schedule, MirPassSchedule, MirPassScheduleError};
pub(crate) use pipeline::{
    run_mir_pipeline_instrumented, run_mir_pipeline_measured, run_mir_pipeline_with_occurrences,
    MeasuredMirPipeline, MirPipelineStatistics,
};
pub use redundancy::{
    analyze_local_primitive_common_subexpressions,
    analyze_proof_local_primitive_common_subexpressions, analyze_proof_redundant_primitive_casts,
    analyze_proof_scalar_spill_provenance, analyze_redundant_primitive_casts,
    analyze_scalar_spill_provenance, LocalCseBlocker, LocalCseCallableObservation,
    LocalCseConsumer, LocalCseCount, LocalCseExcludedFamily, LocalCseObservation,
    LocalCseObservationCounts, LocalCseOperationFamily, LocalCseOutcome, PrimitiveCastBlocker,
    PrimitiveCastCallableObservation, PrimitiveCastConsumer, PrimitiveCastCount,
    PrimitiveCastDisposition, PrimitiveCastObservation, PrimitiveCastObservationCounts,
    PrimitiveCastShape, RedundancySiteClassification, RedundancySiteExample, ScalarSpillBlocker,
    ScalarSpillCallableObservation, ScalarSpillConsumer, ScalarSpillCount, ScalarSpillDepth,
    ScalarSpillProvenanceCounts, ScalarSpillProvenanceObservation, ScalarSpillUnlock,
    REDUNDANCY_SITE_EXAMPLES_PER_CLASSIFICATION,
};
