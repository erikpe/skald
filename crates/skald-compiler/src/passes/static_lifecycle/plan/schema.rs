//! Deterministic conversion from the analyzed plan to lifecycle MIR.

use std::collections::BTreeMap;

use crate::mir::{
    MirProgramLifecycle, MirStaticFieldInitialization, MirStaticLifecycleDefinition,
    MirStaticLifecycleIndices, MirStaticLifecycleProof, MirStaticLifecycleTransition,
    MirStaticLifecycleTransitionKind, PreliminaryMirProgram, StaticEffectAnalysis,
    StaticLifecycleAuthority, StaticLifecyclePlan, StaticLifetimeDependency,
};

use super::model::{PlannedMirProgram, StaticLifecyclePlanningReport};

pub(super) fn build_planned_program(
    mut preliminary: PreliminaryMirProgram,
    authority: StaticLifecycleAuthority,
    effects: StaticEffectAnalysis,
    dependencies: Vec<StaticLifetimeDependency>,
    plan: StaticLifecyclePlan,
) -> PlannedMirProgram {
    let indices = plan
        .activation()
        .iter()
        .copied()
        .enumerate()
        .map(|(activation, field)| {
            (
                field,
                MirStaticLifecycleIndices {
                    activation,
                    shutdown: plan.shutdown().len() - activation - 1,
                },
            )
        })
        .collect::<BTreeMap<_, _>>();

    for (field, lifecycle) in &indices {
        preliminary
            .program_mut()
            .static_field_mut(*field)
            .expect("planned static field must be declared")
            .lifecycle = Some(*lifecycle);
    }

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
            indices: indices[&field.field],
            span: field.span,
        })
        .collect();
    let activation = plan
        .activation()
        .iter()
        .flat_map(|field| {
            let declaration = preliminary
                .static_fields()
                .find(|declaration| declaration.field == *field)
                .expect("planned static field must be inventoried");
            let begin_span = declaration
                .initializer
                .and_then(|initializer| preliminary.static_initializer(initializer))
                .map_or(declaration.span, |initializer| initializer.span);
            let publish = MirStaticLifecycleTransition {
                field: *field,
                kind: if declaration.initializer.is_some() {
                    MirStaticLifecycleTransitionKind::PublishLive
                } else {
                    MirStaticLifecycleTransitionKind::ActivateZeroDefault
                },
                span: declaration
                    .initializer
                    .and_then(|initializer| preliminary.static_initializer(initializer))
                    .map_or(declaration.span, |initializer| initializer.publication.span),
            };
            declaration.initializer.map_or_else(
                || vec![publish],
                |_| {
                    vec![
                        MirStaticLifecycleTransition {
                            field: *field,
                            kind: MirStaticLifecycleTransitionKind::BeginInitialization,
                            span: begin_span,
                        },
                        publish,
                    ]
                },
            )
        })
        .collect();
    let shutdown = plan
        .shutdown()
        .iter()
        .flat_map(|field| {
            let span = preliminary
                .static_fields()
                .find(|declaration| declaration.field == *field)
                .expect("planned static field must be inventoried")
                .span;
            [
                MirStaticLifecycleTransition {
                    field: *field,
                    kind: MirStaticLifecycleTransitionKind::BeginDestruction,
                    span,
                },
                MirStaticLifecycleTransition {
                    field: *field,
                    kind: MirStaticLifecycleTransitionKind::FinishDestruction,
                    span,
                },
            ]
        })
        .collect();
    let proof = MirStaticLifecycleProof::new(authority);
    let lifecycle = MirProgramLifecycle::new(definitions, activation, shutdown, plan, proof);
    let report = StaticLifecyclePlanningReport::new(effects, dependencies);
    PlannedMirProgram::new(preliminary, lifecycle, report)
}
