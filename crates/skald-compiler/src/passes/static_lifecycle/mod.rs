//! Whole-program static-field effect inference over preliminary MIR.
//!
//! This pass owns target-independent call/lifecycle graph construction,
//! transitive effect propagation, static dependency planning, and source
//! diagnostics before lifecycle MIR synthesis.

mod dump;
mod extract;
mod model;
mod plan;
mod root_effects;
mod roots;
mod solve;
mod synthesize;
mod verify;

pub use dump::dump_static_effects;
pub use model::{
    StaticAccessEvidence, StaticAccessKind, StaticArrayLifecycleOperation,
    StaticClassLifecycleOperation, StaticEffectAnalysis, StaticEffectEdge, StaticEffectEdgeKind,
    StaticEffectNode, StaticEffectPhase, StaticEffectSummary, StaticFunctionValueCandidates,
    StaticFunctionValueTarget,
};
pub use plan::{
    dump_planned_mir, dump_static_lifetime_plan, plan_static_lifetimes, PlannedMirProgram,
    StaticLifecyclePlan, StaticLifecyclePlanningFailure, StaticLifetimeDependency,
    StaticLifetimeEvidence, StaticLifetimePhase, STATIC_LIFECYCLE_DEPENDENCY_CYCLE,
    STATIC_LIFECYCLE_SELF_DEPENDENCY,
};
pub use synthesize::synthesize_static_lifecycle;
pub use verify::{verify_planned_mir, verify_synthesized_mir};

use crate::mir::PreliminaryMirProgram;

fn infer_static_effects_with_roots(
    program: &PreliminaryMirProgram,
) -> (
    StaticEffectAnalysis,
    root_effects::StaticLifecycleRootEffectAnalysis,
) {
    let graph = extract::extract(program);
    let root_effects = root_effects::analyze(program, &graph)
        .expect("verified preliminary MIR must have valid lifecycle-root identities");
    let effects = solve::solve(graph);
    debug_assert_eq!(
        root_effects::project_solved_analysis(&root_effects, &effects)
            .expect("solved analysis must cover every lifecycle root"),
        root_effects,
        "checker-oriented root effects must agree with solved summaries"
    );
    (effects, root_effects)
}

/// Infers direct and transitive static-field effects for every executable MIR
/// body and every compiler-generated lifecycle operation in the closed program.
pub fn infer_static_effects(program: &PreliminaryMirProgram) -> StaticEffectAnalysis {
    solve::solve(extract::extract(program))
}

#[cfg(test)]
mod tests;
