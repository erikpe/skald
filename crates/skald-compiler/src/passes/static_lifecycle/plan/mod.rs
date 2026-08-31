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
    identity::StaticFieldId,
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
    let active_fields = preliminary
        .static_fields()
        .map(|field| field.field)
        .collect::<Vec<_>>();
    plan_static_lifetimes_for_active_fields(preliminary, active_fields)
}

fn plan_static_lifetimes_for_active_fields(
    preliminary: PreliminaryMirProgram,
    active_fields: Vec<StaticFieldId>,
) -> Result<PlannedMirProgram, StaticLifecyclePlanningFailure> {
    let activation_authority = StaticActivationAuthority::new(active_fields);
    let dependencies = extract_preliminary_dependencies(&preliminary)
        .expect("verified preliminary MIR must have valid dependency identities");
    // Compute the frozen semantic selection at its permanent boundary, but do
    // not let it narrow lifecycle planning until subset proof is available.
    let shadow_activation =
        analyze_static_activation_from_dependencies(&preliminary, &dependencies)
            .expect("verified preliminary MIR must have a valid static activation closure");
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
        shadow_activation,
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
pub(crate) fn plan_static_lifetimes_for_fields_for_test(
    preliminary: PreliminaryMirProgram,
    active_fields: Vec<StaticFieldId>,
) -> Result<PlannedMirProgram, StaticLifecyclePlanningFailure> {
    plan_static_lifetimes_for_active_fields(preliminary, active_fields)
}

#[cfg(test)]
mod tests;
