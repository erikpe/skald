//! Whole-program static-field effect inference over preliminary MIR.
//!
//! This pass owns target-independent call/lifecycle graph construction,
//! transitive effect propagation, static dependency planning, and source
//! diagnostics before lifecycle MIR synthesis.

mod dump;
mod extract;
mod model;
mod plan;
mod roots;
mod solve;
mod verify;

pub use dump::dump_static_effects;
pub use model::{
    StaticAccessEvidence, StaticAccessKind, StaticArrayLifecycleOperation,
    StaticClassLifecycleOperation, StaticEffectAnalysis, StaticEffectEdge, StaticEffectEdgeKind,
    StaticEffectNode, StaticEffectPhase, StaticEffectSummary,
};
pub use plan::{
    dump_planned_mir, dump_static_lifetime_plan, plan_static_lifetimes, PlannedMirProgram,
    StaticLifecyclePlan, StaticLifecyclePlanningFailure, StaticLifetimeDependency,
    StaticLifetimeEvidence, StaticLifetimePhase, STATIC_LIFECYCLE_DEPENDENCY_CYCLE,
    STATIC_LIFECYCLE_SELF_DEPENDENCY,
};
pub use verify::verify_planned_mir;

use crate::mir::PreliminaryMirProgram;

/// Infers direct and transitive static-field effects for every executable MIR
/// body and every compiler-generated lifecycle operation in the closed program.
pub fn infer_static_effects(program: &PreliminaryMirProgram) -> StaticEffectAnalysis {
    solve::solve(extract::extract(program))
}

#[cfg(test)]
mod tests;
