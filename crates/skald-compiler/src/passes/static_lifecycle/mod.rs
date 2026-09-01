//! Static-lifecycle analysis, planning, synthesis, and verification facade.
//!
//! Implementation details remain with their phase owners while this module
//! preserves the supported cross-phase API.

mod analysis;
// Keep the complete query and dump surface private while production computes
// it in shadow mode; later lifecycle products will carry its exact authority
// across phase boundaries.
#[allow(dead_code)]
mod activation;
mod plan;
mod synthesize;
mod verify;

pub use crate::mir::{
    StaticAccessKind, StaticArrayLifecycleOperation, StaticClassLifecycleOperation,
    StaticEffectNode, StaticEffectPhase,
};
pub use crate::mir::{
    StaticActivationAuthority, StaticLifecycleAuthority, StaticLifecycleEffectFact,
    StaticLifecycleRootAuthority,
};
pub use analysis::{
    dump_static_effects, infer_static_effects, StaticAccessEvidence, StaticEffectAnalysis,
    StaticEffectEdge, StaticEffectEdgeKind, StaticEffectSummary, StaticFunctionValueCandidates,
    StaticFunctionValueTarget,
};
#[cfg(test)]
pub(crate) use plan::plan_static_lifetimes_for_fields_for_test;
pub use plan::{
    dump_planned_mir, dump_static_lifetime_plan, plan_static_lifetimes, PlannedMirProgram,
    StaticLifecyclePlan, StaticLifecyclePlanningFailure, StaticLifecyclePlanningReport,
    StaticLifetimeDependency, StaticLifetimeEvidence, StaticLifetimePhase,
    STATIC_LIFECYCLE_DEPENDENCY_CYCLE, STATIC_LIFECYCLE_SELF_DEPENDENCY,
};
pub use synthesize::synthesize_static_lifecycle;
pub use verify::{verify_planned_mir, verify_synthesized_mir, VerifiedPlannedMirProgram};
