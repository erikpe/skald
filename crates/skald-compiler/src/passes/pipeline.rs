//! Target-independent MIR pass registration, execution, and accounting.

use std::ops::Deref;

use crate::mir::{MirProgram, MirVerificationErrors};

use super::static_lifecycle;

mod execution;
// Exact internal schedules and constructors remain compiler/test tooling until
// the first production pass is registered.
#[allow(dead_code)]
mod policy;

pub(crate) use execution::{
    run_mir_pipeline_measured, run_mir_pipeline_with_occurrences, MeasuredMirPipeline,
    MirPipelineStatistics,
};
pub use execution::{
    MirPassMeasurement, MirPassOccurrenceOutcome, MirPassOccurrenceRecord, MirPipelineError,
    MirPipelineFailureStage,
};
pub use policy::MirOptimizationProfile;
pub use policy::MirPassIdentity;
pub(crate) use policy::{
    registered_mir_pass_names, resolve_exact_mir_pass_schedule, resolve_mir_pass_schedule,
    MirPassOccurrence, MirPassSchedule, MirPassScheduleError,
};

/// Read-only final MIR that passed ordinary and lifecycle-realization checks.
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
/// The pass rewrite capability is also implementation-private:
///
/// ```compile_fail
/// use skald_compiler::passes::MirPassCapability;
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedFinalMirProgram {
    program: MirProgram,
}

impl VerifiedFinalMirProgram {
    pub const fn program(&self) -> &MirProgram {
        &self.program
    }

    /// Invalidates the final-MIR seal for a target-independent transformation.
    ///
    /// Visibility is deliberately restricted to the pass owner. Rewriters and
    /// backends cannot extract raw MIR from a verified product themselves.
    fn invalidate_for_transformation(self) -> MirProgram {
        self.program
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
    let schedule = resolve_mir_pass_schedule(MirOptimizationProfile::Default, std::iter::empty())
        .expect("compiler-owned default MIR pass policy must be valid");
    run_mir_pipeline_measured(program, &schedule).result
}

/// Seals final MIR after the central ordinary and lifecycle-realization check.
///
/// This is the invalidation target for future transformations that can change
/// static accesses, control-flow reachability, lifecycle operations, or
/// possible callees. Passes that affect any of those facts must return raw MIR
/// to this boundary before backend input can be constructed.
pub fn verify_final_mir(
    program: MirProgram,
) -> Result<VerifiedFinalMirProgram, MirVerificationErrors> {
    static_lifecycle::verify_synthesized_mir(&program)?;
    Ok(VerifiedFinalMirProgram { program })
}

#[cfg(test)]
mod tests;
