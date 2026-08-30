//! Static dependency planning after closed-world effect inference.

pub(super) mod derived;
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
        lifecycle,
    ))
}

impl PlannedMirProgram {
    /// Derives deterministic source-rich dependency evidence for inspection.
    /// Semantic dependency pairs remain derived from baseline authority by
    /// verification rather than stored in the accepted phase product.
    pub fn dependencies(&self) -> Vec<StaticLifetimeDependency> {
        graph::LifetimeGraph::build(self.preliminary(), self.planning_report().analysis())
            .dependencies()
            .to_vec()
    }
}

#[cfg(test)]
mod tests;
