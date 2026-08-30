//! Static dependency planning after closed-world effect inference.

mod diagnostics;
mod dump;
mod graph;
mod model;
mod schema;

pub use diagnostics::{STATIC_LIFECYCLE_DEPENDENCY_CYCLE, STATIC_LIFECYCLE_SELF_DEPENDENCY};
pub use dump::{dump_planned_mir, dump_static_lifetime_plan};
pub use model::{
    PlannedMirProgram, StaticLifecyclePlan, StaticLifecyclePlanningFailure,
    StaticLifecyclePlanningReport, StaticLifetimeDependency, StaticLifetimeEvidence,
    StaticLifetimePhase,
};

use crate::{diagnostics::Diagnostics, mir::PreliminaryMirProgram};

use super::analysis::infer_static_effects_with_roots;

/// Infers effects once and converts them into a deterministic whole-program
/// activation and exact-reverse shutdown plan.
pub fn plan_static_lifetimes(
    preliminary: PreliminaryMirProgram,
) -> Result<PlannedMirProgram, StaticLifecyclePlanningFailure> {
    let (effects, root_effects) = infer_static_effects_with_roots(&preliminary);
    let graph = graph::LifetimeGraph::build(&preliminary, &effects);
    let cyclic_components = graph.cyclic_components();
    if !cyclic_components.is_empty() {
        let diagnostics = diagnostics::cycle_diagnostics(&preliminary, &graph, &cyclic_components)
            .into_iter()
            .collect::<Diagnostics>();
        return Err(StaticLifecyclePlanningFailure::new(
            graph.dependencies().to_vec(),
            diagnostics,
        ));
    }

    let lifecycle = graph.plan();
    Ok(schema::build_planned_program(
        preliminary,
        root_effects,
        effects,
        graph.dependencies().to_vec(),
        lifecycle,
    ))
}

#[cfg(test)]
mod tests;
