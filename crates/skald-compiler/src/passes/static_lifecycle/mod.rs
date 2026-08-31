//! Static-lifecycle analysis, planning, synthesis, and verification facade.
//!
//! Implementation details remain with their phase owners while this module
//! preserves the supported cross-phase API.

mod analysis;
// The frozen activation vocabulary lands before its extraction and solver
// consumers. Keeping the additive model private prevents it from becoming a
// second reachability API while the later analysis boundary is assembled.
#[allow(dead_code, unused_imports)]
mod activation;
mod plan;
mod synthesize;
mod verify;

pub use crate::mir::{
    StaticAccessKind, StaticArrayLifecycleOperation, StaticClassLifecycleOperation,
    StaticEffectNode, StaticEffectPhase,
};
pub use crate::mir::{
    StaticLifecycleAuthority, StaticLifecycleEffectFact, StaticLifecycleRootAuthority,
};
pub use analysis::{
    dump_static_effects, infer_static_effects, StaticAccessEvidence, StaticEffectAnalysis,
    StaticEffectEdge, StaticEffectEdgeKind, StaticEffectSummary, StaticFunctionValueCandidates,
    StaticFunctionValueTarget,
};
pub use plan::{
    dump_planned_mir, dump_static_lifetime_plan, plan_static_lifetimes, PlannedMirProgram,
    StaticLifecyclePlan, StaticLifecyclePlanningFailure, StaticLifecyclePlanningReport,
    StaticLifetimeDependency, StaticLifetimeEvidence, StaticLifetimePhase,
    STATIC_LIFECYCLE_DEPENDENCY_CYCLE, STATIC_LIFECYCLE_SELF_DEPENDENCY,
};
pub use synthesize::synthesize_static_lifecycle;
pub use verify::{verify_planned_mir, verify_synthesized_mir, VerifiedPlannedMirProgram};
