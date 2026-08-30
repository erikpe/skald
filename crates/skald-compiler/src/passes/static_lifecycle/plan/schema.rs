//! Deterministic conversion from the analyzed graph to canonical planned MIR.

use crate::mir::{
    MirPlannedLifecycle, MirStaticFieldInitialization, MirStaticLifecycleDefinition,
    MirStaticLifecycleProof, PreliminaryMirProgram, StaticLifecycleAuthority, StaticLifecyclePlan,
};

use super::{
    super::analysis::StaticEffectAnalysis,
    model::{PlannedMirProgram, StaticLifecyclePlanningReport},
};

pub(super) fn build_planned_program(
    preliminary: PreliminaryMirProgram,
    authority: StaticLifecycleAuthority,
    effects: StaticEffectAnalysis,
    plan: StaticLifecyclePlan,
) -> PlannedMirProgram {
    let definitions = preliminary
        .static_fields()
        .map(|field| MirStaticLifecycleDefinition {
            field: field.field,
            ty: field.ty,
            initialization: field.initializer.map_or(
                MirStaticFieldInitialization::ZeroDefault,
                MirStaticFieldInitialization::Explicit,
            ),
            final_span: field.final_span,
            span: field.span,
        })
        .collect();
    let lifecycle =
        MirPlannedLifecycle::new(definitions, plan, MirStaticLifecycleProof::new(authority));
    let report = StaticLifecyclePlanningReport::new(effects);
    PlannedMirProgram::new(preliminary, lifecycle, report)
}
