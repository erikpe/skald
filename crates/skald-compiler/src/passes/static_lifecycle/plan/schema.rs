//! Deterministic conversion from the analyzed plan to lifecycle MIR.

use std::collections::BTreeMap;

use crate::mir::{
    MirProgramLifecycle, MirStaticFieldInitialization, MirStaticLifecycleCertificate,
    MirStaticLifecycleDefinition, MirStaticLifecycleIndices, MirStaticLifecycleTransition,
    MirStaticLifecycleTransitionKind, PlannedMirProgram, PreliminaryMirProgram,
    StaticEffectAnalysis, StaticLifecyclePlan, StaticLifetimeDependency,
};

pub(super) fn build_planned_program(
    mut preliminary: PreliminaryMirProgram,
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
            [
                MirStaticLifecycleTransition {
                    field: *field,
                    kind: MirStaticLifecycleTransitionKind::BeginInitialization,
                    span: begin_span,
                },
                MirStaticLifecycleTransition {
                    field: *field,
                    kind: MirStaticLifecycleTransitionKind::PublishLive,
                    span: declaration
                        .initializer
                        .and_then(|initializer| preliminary.static_initializer(initializer))
                        .map_or(declaration.span, |initializer| initializer.publication.span),
                },
            ]
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
    let certificate = MirStaticLifecycleCertificate::new(effects, dependencies);
    let lifecycle = MirProgramLifecycle::new(definitions, activation, shutdown, plan, certificate);
    PlannedMirProgram::new(preliminary, lifecycle)
}
