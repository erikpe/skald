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

use crate::{
    diagnostics::Diagnostics,
    mir::{PreliminaryMirProgram, StaticActivationAuthority},
    passes::reachability::extract_preliminary_dependencies,
};

use super::{
    activation::analyze_static_activation_from_dependencies,
    analysis::infer_static_effects_with_roots_for_fields_from_dependencies,
};

/// Infers effects once and converts them into a deterministic whole-program
/// activation and exact-reverse shutdown plan.
pub fn plan_static_lifetimes(
    preliminary: PreliminaryMirProgram,
) -> Result<PlannedMirProgram, StaticLifecyclePlanningFailure> {
    let dependencies = extract_preliminary_dependencies(&preliminary)
        .expect("verified preliminary MIR must have valid dependency identities");
    let activation = analyze_static_activation_from_dependencies(&preliminary, &dependencies)
        .expect("verified preliminary MIR must have a valid static activation closure");
    let activation_authority = StaticActivationAuthority::new(
        activation
            .active_fields()
            .iter()
            .map(|active| active.field())
            .collect(),
    );
    let (effects, root_effects) = infer_static_effects_with_roots_for_fields_from_dependencies(
        &preliminary,
        &dependencies,
        activation_authority.fields(),
    );
    let graph = graph::LifetimeGraph::build_for_fields(
        &preliminary,
        &effects,
        activation_authority.fields().to_vec(),
    );
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
        activation_authority,
        root_effects,
        effects,
        activation,
        lifecycle,
    ))
}

impl PlannedMirProgram {
    /// Derives deterministic source-rich dependency evidence for inspection.
    /// Semantic dependency pairs remain derived from baseline authority by
    /// verification rather than stored in the accepted phase product.
    pub fn dependencies(&self) -> Vec<StaticLifetimeDependency> {
        graph::LifetimeGraph::build_for_fields(
            self.preliminary(),
            self.planning_report().analysis(),
            self.activation_authority().fields().to_vec(),
        )
        .dependencies()
        .to_vec()
    }
}

#[cfg(test)]
mod tests;
