//! Target-independent MIR pass registration, execution, and accounting.

use crate::mir::{MirProgram, MirVerificationErrors};

use super::reachability::MirDependencyExtractionError;

mod execution;
mod normalization;
mod optimizations;
mod policy;
mod seal;

pub(in crate::passes) use optimizations::{
    evaluate_integer_division, evaluate_rvalue, evaluate_shift, CheckedIntegerEvaluation,
    PrimitiveConstant, PrimitiveEvaluation,
};

pub(crate) use execution::{
    run_mir_pipeline_instrumented, run_mir_pipeline_measured, run_mir_pipeline_with_occurrences,
    MeasuredMirPipeline, MirPipelineStatistics,
};
#[cfg(test)]
pub(in crate::passes::pipeline) use execution::{
    run_mir_pipeline_measured_inspected, run_mir_pipeline_with_transition_and_occurrences_for_test,
    run_mir_pipeline_with_transition_for_test,
};
pub use execution::{
    MirFinalPipelineCheckpoint, MirPassMeasurement, MirPassOccurrenceOutcome,
    MirPassOccurrenceRecord, MirPipelineCheckpoint, MirPipelineCheckpointLabel, MirPipelineError,
    MirPipelineFailureStage, MirPipelineInspector, MirProofPipelineCheckpoint,
};
pub use policy::{
    available_mir_passes, MirOptimizationProfile, MirPassDescriptor, MirPassIdentity, MirPassStage,
};
pub(crate) use policy::{
    resolve_exact_mir_pass_schedule, resolve_mir_pass_schedule, MirPassOccurrence, MirPassSchedule,
    MirPassScheduleError,
};
pub(crate) use seal::verify_proof_mir;
pub use seal::{verify_final_mir, VerifiedFinalMirProgram, VerifiedProofMirProgram};

/// Runs the target-independent MIR pass pipeline.
///
/// The selected default schedule is resolved explicitly and executed by the
/// same verified runner used by request compilation. The returned sealed
/// product is the only MIR accepted by backend input.
pub fn run_mir_pipeline(program: MirProgram) -> Result<VerifiedFinalMirProgram, MirPipelineError> {
    let schedule = default_mir_pass_schedule();
    run_mir_pipeline_measured(program, &schedule).result
}

/// Runs the default final-MIR pipeline with verified inspection checkpoints.
///
/// The inspector receives the proof-rich input, every successfully completed
/// pass occurrence, the mandatory normalization boundary, and the normalized
/// final product. Ordinary compilation uses [`run_mir_pipeline`] and performs
/// no checkpoint work.
pub fn run_mir_pipeline_inspected(
    program: MirProgram,
    inspector: &mut dyn MirPipelineInspector,
) -> Result<VerifiedFinalMirProgram, MirPipelineError> {
    let schedule = default_mir_pass_schedule();
    run_mir_pipeline_instrumented(program, &schedule, false, Some(inspector)).result
}

fn default_mir_pass_schedule() -> MirPassSchedule {
    resolve_mir_pass_schedule(MirOptimizationProfile::Default, std::iter::empty())
        .expect("compiler-owned default MIR pass policy must be valid")
}

fn reachability_verification_errors(error: MirDependencyExtractionError) -> MirVerificationErrors {
    MirVerificationErrors::program(format!("reachability analysis failed: {error}"))
}

#[cfg(test)]
#[path = "pipeline/composition_tests.rs"]
mod composition_tests;
#[cfg(test)]
mod seal_tests;
#[cfg(test)]
#[path = "pipeline/source_equivalence_tests.rs"]
mod source_equivalence_tests;
#[cfg(test)]
mod tests;
