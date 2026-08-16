//! Structural verification for lifecycle definitions, transitions, and order.

use std::collections::{BTreeMap, BTreeSet};

use crate::mir::{
    MirStaticFieldInitialization, MirStaticLifecycleTransitionKind, MirVerificationError,
    PlannedMirProgram,
};

use super::program_error;

pub(super) fn verify(program: &PlannedMirProgram, errors: &mut Vec<MirVerificationError>) {
    let preliminary = program.preliminary();
    let fields = preliminary.static_fields().copied().collect::<Vec<_>>();
    let lifecycle = program.lifecycle_mir();
    let activation = lifecycle.plan().activation();
    let shutdown = lifecycle.plan().shutdown();

    let declared = fields
        .iter()
        .map(|field| field.field)
        .collect::<BTreeSet<_>>();
    verify_order("activation", activation, &declared, errors);
    verify_order("shutdown", shutdown, &declared, errors);
    if shutdown
        .iter()
        .copied()
        .ne(activation.iter().rev().copied())
    {
        program_error(
            errors,
            "static shutdown order is not the exact reverse of activation",
        );
    }

    let activation_indices = activation
        .iter()
        .copied()
        .enumerate()
        .map(|(index, field)| (field, index))
        .collect::<BTreeMap<_, _>>();
    let shutdown_indices = shutdown
        .iter()
        .copied()
        .enumerate()
        .map(|(index, field)| (field, index))
        .collect::<BTreeMap<_, _>>();

    let mut definitions = BTreeMap::new();
    for definition in lifecycle.definitions() {
        if definitions.insert(definition.field, definition).is_some() {
            program_error(
                errors,
                format!("duplicate lifecycle definition for {}", definition.field),
            );
        }
    }
    if definitions.len() != fields.len() {
        program_error(
            errors,
            "lifecycle definition table does not cover every static field",
        );
    }

    for field in &fields {
        let Some(declaration) = preliminary.program().static_field(field.field) else {
            program_error(
                errors,
                format!("static inventory names undeclared field {}", field.field),
            );
            continue;
        };
        let mode = field.initializer.map_or(
            MirStaticFieldInitialization::ZeroDefault,
            MirStaticFieldInitialization::Explicit,
        );
        if declaration.initialization != mode {
            program_error(
                errors,
                format!(
                    "static field {} has an inconsistent initialization mode",
                    field.field
                ),
            );
        }
        if field.final_span.is_some() && field.initializer.is_none() {
            program_error(
                errors,
                format!(
                    "final static field {} cannot use zero-default lifecycle activation",
                    field.field
                ),
            );
        }
        let expected_indices = activation_indices
            .get(&field.field)
            .zip(shutdown_indices.get(&field.field))
            .map(
                |(activation, shutdown)| crate::mir::MirStaticLifecycleIndices {
                    activation: *activation,
                    shutdown: *shutdown,
                },
            );
        if declaration.lifecycle != expected_indices {
            program_error(
                errors,
                format!(
                    "static field {} has inconsistent lifecycle indices",
                    field.field
                ),
            );
        }

        let Some(definition) = definitions.get(&field.field) else {
            continue;
        };
        if definition.ty != field.ty
            || definition.initialization != mode
            || definition.final_span != field.final_span
            || Some(definition.indices) != expected_indices
            || definition.span != field.span
        {
            program_error(
                errors,
                format!(
                    "lifecycle definition for {} disagrees with its declaration",
                    field.field
                ),
            );
        }
    }
    for field in definitions.keys() {
        if !declared.contains(field) {
            program_error(
                errors,
                format!("lifecycle definition names foreign static field {field}"),
            );
        }
    }

    verify_activation_transitions(program, errors);
    verify_shutdown_transitions(program, errors);
}

fn verify_order(
    name: &str,
    order: &[crate::identity::StaticFieldId],
    declared: &BTreeSet<crate::identity::StaticFieldId>,
    errors: &mut Vec<MirVerificationError>,
) {
    let unique = order.iter().copied().collect::<BTreeSet<_>>();
    if order.len() != declared.len() || unique != *declared {
        program_error(
            errors,
            format!("static {name} order does not cover every field exactly once"),
        );
    }
}

fn verify_activation_transitions(
    program: &PlannedMirProgram,
    errors: &mut Vec<MirVerificationError>,
) {
    let transitions = program.lifecycle_mir().activation();
    let order = program.lifecycle().activation();
    let mut transitions = transitions.iter();
    for field in order {
        let declaration = program
            .preliminary()
            .static_fields()
            .find(|candidate| candidate.field == *field);
        let Some(declaration) = declaration else {
            continue;
        };
        let initializer = declaration
            .initializer
            .and_then(|id| program.preliminary().static_initializer(id));
        let expected_begin_span = initializer.map_or(declaration.span, |body| body.span);
        let expected_publish_span =
            initializer.map_or(declaration.span, |body| body.publication.span);
        let valid = if initializer.is_some() {
            matches!(
                (transitions.next(), transitions.next()),
                (Some(begin), Some(publish))
                    if begin.field == *field
                        && begin.kind == MirStaticLifecycleTransitionKind::BeginInitialization
                        && begin.span == expected_begin_span
                        && publish.field == *field
                        && publish.kind == MirStaticLifecycleTransitionKind::PublishLive
                        && publish.span == expected_publish_span
            )
        } else {
            matches!(
                transitions.next(),
                Some(activate)
                    if activate.field == *field
                        && activate.kind == MirStaticLifecycleTransitionKind::ActivateZeroDefault
                        && activate.span == expected_publish_span
            )
        };
        if !valid {
            program_error(
                errors,
                format!("activation phase partition for {field} is malformed"),
            );
        }
    }
    if transitions.next().is_some() {
        program_error(
            errors,
            "static activation transition table has trailing phases",
        );
    }
}

fn verify_shutdown_transitions(
    program: &PlannedMirProgram,
    errors: &mut Vec<MirVerificationError>,
) {
    let transitions = program.lifecycle_mir().shutdown();
    let order = program.lifecycle().shutdown();
    if transitions.len() != order.len() * 2 {
        program_error(
            errors,
            "static shutdown transition table has incomplete phase coverage",
        );
        return;
    }
    for (field, pair) in order.iter().zip(transitions.chunks_exact(2)) {
        let span = program
            .preliminary()
            .static_fields()
            .find(|candidate| candidate.field == *field)
            .map(|declaration| declaration.span);
        if pair[0].field != *field
            || pair[0].kind != MirStaticLifecycleTransitionKind::BeginDestruction
            || Some(pair[0].span) != span
            || pair[1].field != *field
            || pair[1].kind != MirStaticLifecycleTransitionKind::FinishDestruction
            || Some(pair[1].span) != span
        {
            program_error(
                errors,
                format!("shutdown phase partition for {field} is malformed"),
            );
        }
    }
}
