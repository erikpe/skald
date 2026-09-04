//! Target-independent MIR pass registration, execution, and accounting.

use std::{fmt, ops::Deref};

use crate::mir::{MirProgram, MirVerificationErrors};

use super::{
    reachability::{
        analyze_reachability, verify_active_lifecycle_reachability, verify_reachable_definitions,
        verify_reachable_static_accesses, MirDependencyExtractionError, MirReachabilityAnalysis,
    },
    static_lifecycle,
};

mod execution;
mod optimizations;
mod policy;

#[cfg(test)]
pub(crate) use execution::run_mir_pipeline_measured_inspected;
pub(crate) use execution::{
    run_mir_pipeline_instrumented, run_mir_pipeline_measured, run_mir_pipeline_with_occurrences,
    MeasuredMirPipeline, MirPipelineStatistics,
};
pub use execution::{
    MirPassMeasurement, MirPassOccurrenceOutcome, MirPassOccurrenceRecord, MirPipelineCheckpoint,
    MirPipelineCheckpointLabel, MirPipelineError, MirPipelineFailureStage, MirPipelineInspector,
};
pub use policy::{
    available_mir_passes, MirOptimizationProfile, MirPassDescriptor, MirPassIdentity,
};
pub(crate) use policy::{
    resolve_exact_mir_pass_schedule, resolve_mir_pass_schedule, MirPassOccurrence, MirPassSchedule,
    MirPassScheduleError,
};

/// Read-only final MIR with verified structure, lifecycle realization, and
/// target-independent reachability facts.
///
/// The private representation is the backend trust token. Any future pass
/// that changes executable MIR must produce raw MIR and call [`verify_final_mir`]
/// before constructing backend input again.
///
/// External code cannot forge the seal:
///
/// ```compile_fail
/// use skald_compiler::{mir::MirProgram, passes::VerifiedFinalMirProgram};
///
/// fn forge(program: MirProgram) -> VerifiedFinalMirProgram {
///     VerifiedFinalMirProgram { program }
/// }
/// ```
///
/// Seal-bound reachability facts are implementation-private and cannot be
/// detached, replaced, or mutated by external code:
///
/// ```compile_fail
/// use skald_compiler::passes::VerifiedFinalMirProgram;
///
/// fn detach(verified: &VerifiedFinalMirProgram) {
///     let _ = verified.reachability();
/// }
/// ```
///
/// ```compile_fail
/// use skald_compiler::passes::VerifiedFinalMirProgram;
///
/// fn replace_facts(verified: &mut VerifiedFinalMirProgram) {
///     verified.reachability = verified.reachability.clone();
/// }
/// ```
///
/// The pass rewrite capability is also implementation-private:
///
/// ```compile_fail
/// use skald_compiler::passes::MirPassCapability;
/// ```
#[derive(Clone, Eq, PartialEq)]
pub struct VerifiedFinalMirProgram {
    program: MirProgram,
    reachability: Box<MirReachabilityAnalysis>,
}

impl VerifiedFinalMirProgram {
    pub const fn program(&self) -> &MirProgram {
        &self.program
    }

    #[allow(dead_code)]
    pub(crate) const fn reachability(&self) -> &MirReachabilityAnalysis {
        &self.reachability
    }

    /// Invalidates the final-MIR seal for a target-independent transformation.
    ///
    /// Visibility is deliberately restricted to the pass owner. Rewriters and
    /// backends cannot extract raw MIR from a verified product themselves.
    fn invalidate_for_transformation(self) -> MirProgram {
        let Self {
            program,
            reachability: _,
        } = self;
        program
    }
}

impl fmt::Debug for VerifiedFinalMirProgram {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedFinalMirProgram")
            .field("program", &self.program)
            .finish()
    }
}

impl Deref for VerifiedFinalMirProgram {
    type Target = MirProgram;

    fn deref(&self) -> &Self::Target {
        self.program()
    }
}

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
/// The inspector receives `input`, every successfully completed pass
/// occurrence, and `final`. Ordinary compilation uses [`run_mir_pipeline`]
/// and performs no checkpoint work.
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

/// Seals final MIR after central ordinary, lifecycle-realization, and
/// target-independent reachability analysis.
///
/// This is the invalidation target for future transformations that can change
/// static accesses, control-flow reachability, lifecycle operations, or
/// possible callees. Passes that affect any of those facts must return raw MIR
/// to this boundary before backend input can be constructed.
pub fn verify_final_mir(
    program: MirProgram,
) -> Result<VerifiedFinalMirProgram, MirVerificationErrors> {
    static_lifecycle::verify_synthesized_mir(&program)?;
    let reachability = analyze_reachability(&program).map_err(reachability_verification_errors)?;
    verify_reachable_definitions(&program, &reachability)?;
    verify_active_lifecycle_reachability(&program, &reachability)?;
    verify_reachable_static_accesses(&program, &reachability)?;
    Ok(VerifiedFinalMirProgram {
        program,
        reachability: Box::new(reachability),
    })
}

fn reachability_verification_errors(error: MirDependencyExtractionError) -> MirVerificationErrors {
    MirVerificationErrors::program(format!("reachability analysis failed: {error}"))
}

#[cfg(test)]
mod seal_tests;
#[cfg(test)]
mod tests;
