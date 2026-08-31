//! Deterministic conversion from the analyzed graph to canonical planned MIR.

use crate::mir::{
    MirPlannedLifecycle, MirStaticFieldInitialization, MirStaticLifecycleDefinition,
    MirStaticLifecycleProof, PreliminaryMirProgram, StaticActivationAuthority,
    StaticLifecycleAuthority, StaticLifecyclePlan,
};

use super::{
    super::{activation::StaticActivationAnalysis, analysis::StaticEffectAnalysis},
    model::{PlannedMirProgram, StaticLifecyclePlanningReport},
};

pub(super) fn build_planned_program(
    preliminary: PreliminaryMirProgram,
    activation_authority: StaticActivationAuthority,
    authority: StaticLifecycleAuthority,
    effects: StaticEffectAnalysis,
    activation: StaticActivationAnalysis,
    plan: StaticLifecyclePlan,
) -> PlannedMirProgram {
    let definitions = preliminary
        .static_fields()
        .filter(|field| activation_authority.contains(field.field))
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
    let lifecycle = MirPlannedLifecycle::new(
        definitions,
        plan,
        MirStaticLifecycleProof::new(activation_authority, authority),
    );
    let report = StaticLifecyclePlanningReport::new(effects, activation);
    PlannedMirProgram::new(preliminary, lifecycle, report)
}
