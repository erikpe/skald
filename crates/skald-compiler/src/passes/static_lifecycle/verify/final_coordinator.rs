//! Structural and control-flow verification for synthesized lifecycle MIR.

use std::collections::{BTreeMap, BTreeSet};

use crate::mir::{
    destination_completed_on_every_publication_path, reachable_static_initializer_blocks,
    MirStaticActivationWork, MirStaticFieldInitialization, MirStaticInitializerBody,
    MirStaticLifecycleCoordinator, MirStaticLifecycleTransitionKind, MirStaticValueCleanup,
    MirTerminator, MirVerificationError,
};

use super::{program_error, LifecycleMirView};

pub(super) fn verify(
    view: LifecycleMirView<'_>,
    coordinator: &MirStaticLifecycleCoordinator,
    errors: &mut Vec<MirVerificationError>,
) {
    verify_definitions(view, errors);
    verify_activation(view, coordinator, errors);
    verify_shutdown(view, coordinator, errors);
}

fn verify_definitions(view: LifecycleMirView<'_>, errors: &mut Vec<MirVerificationError>) {
    let declarations = view
        .program
        .classes
        .iter()
        .flat_map(|class| &class.static_fields)
        .collect::<Vec<_>>();
    let definitions = view.lifecycle.definitions();
    if definitions.len() != declarations.len() {
        program_error(
            errors,
            "final static lifecycle definitions do not cover every field",
        );
    }
    let mut fields = BTreeSet::new();
    for pair in definitions.windows(2) {
        match pair[0].field.cmp(&pair[1].field) {
            std::cmp::Ordering::Equal => program_error(
                errors,
                format!("duplicate final lifecycle definition for {}", pair[0].field),
            ),
            std::cmp::Ordering::Greater => program_error(
                errors,
                "final lifecycle definitions are not in canonical field order",
            ),
            std::cmp::Ordering::Less => {}
        }
    }
    for definition in definitions {
        if !fields.insert(definition.field) {
            program_error(
                errors,
                format!(
                    "duplicate final lifecycle definition for {}",
                    definition.field
                ),
            );
        }
        let Some(declaration) = view.program.static_field(definition.field) else {
            program_error(
                errors,
                format!(
                    "final lifecycle definition names foreign field {}",
                    definition.field
                ),
            );
            continue;
        };
        if declaration.ty != definition.ty
            || declaration.initialization != definition.initialization
            || declaration.final_span != definition.final_span
            || declaration.span != definition.span
        {
            program_error(
                errors,
                format!(
                    "final lifecycle definition for {} disagrees with its declaration",
                    definition.field
                ),
            );
        }
        if definition.final_span.is_some()
            && !matches!(
                definition.initialization,
                MirStaticFieldInitialization::Explicit(_)
            )
        {
            program_error(
                errors,
                format!(
                    "final lifecycle definition for {} must publish one explicit initializer",
                    definition.field
                ),
            );
        }
    }
    let planned = view
        .lifecycle
        .plan()
        .activation()
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if planned != fields
        || view.lifecycle.plan().activation().len() != fields.len()
        || view.lifecycle.plan().shutdown().ne(view
            .lifecycle
            .plan()
            .activation()
            .iter()
            .rev()
            .copied())
    {
        program_error(
            errors,
            "final lifecycle plan lacks exact activation and reverse-shutdown coverage",
        );
    }
}

fn verify_activation(
    view: LifecycleMirView<'_>,
    coordinator: &MirStaticLifecycleCoordinator,
    errors: &mut Vec<MirVerificationError>,
) {
    let order = view.lifecycle.plan().activation();
    if coordinator.activation().len() != order.len() {
        program_error(errors, "final activation regions do not cover every field");
    }
    let initializers = coordinator
        .initializers()
        .iter()
        .map(|initializer| (initializer.id, initializer))
        .collect::<BTreeMap<_, _>>();
    if initializers.len() != coordinator.initializers().len() {
        program_error(
            errors,
            "final coordinator contains duplicate initializer bodies",
        );
    }

    for (index, region) in coordinator.activation().iter().enumerate() {
        let Some(expected_field) = order.get(index) else {
            break;
        };
        if region.field != *expected_field {
            program_error(errors, "final activation regions are reordered");
            continue;
        }
        let Some(definition) = view.lifecycle.definition(*expected_field) else {
            continue;
        };
        match (definition.initialization, region.work) {
            (MirStaticFieldInitialization::ZeroDefault, MirStaticActivationWork::ZeroDefault) => {
                if !matches!(
                    region.transitions.as_slice(),
                    [activate]
                        if activate.field == *expected_field
                            && activate.kind
                                == MirStaticLifecycleTransitionKind::ActivateZeroDefault
                            && activate.span == definition.span
                ) {
                    program_error(
                        errors,
                        format!("zero-default activation for {expected_field} is malformed"),
                    );
                }
            }
            (
                MirStaticFieldInitialization::Explicit(expected),
                MirStaticActivationWork::Explicit(actual),
            ) if expected == actual => {
                let Some(initializer) = initializers.get(&expected).copied() else {
                    // Sparse final MIR may omit a body. Reachability
                    // completeness independently rejects this rooted target;
                    // coordinator structure can still validate its declared
                    // activation identity here.
                    continue;
                };
                if initializer.field != *expected_field
                    || initializer.destination_type != definition.ty
                {
                    program_error(
                        errors,
                        format!("initializer body for {expected_field} has the wrong destination"),
                    );
                }
                if !matches!(
                    region.transitions.as_slice(),
                    [begin, publish]
                        if begin.field == *expected_field
                            && begin.kind
                                == MirStaticLifecycleTransitionKind::BeginInitialization
                            && begin.span == initializer.span
                            && publish.field == *expected_field
                            && publish.kind == MirStaticLifecycleTransitionKind::PublishLive
                            && publish.span == initializer.publication.span
                ) {
                    program_error(
                        errors,
                        format!("explicit activation for {expected_field} is malformed"),
                    );
                }
                verify_publication(initializer, errors);
            }
            _ => program_error(
                errors,
                format!("activation work for {expected_field} has the wrong mode"),
            ),
        }
    }
    let expected_initializers = view
        .lifecycle
        .definitions()
        .iter()
        .filter_map(|definition| match definition.initialization {
            MirStaticFieldInitialization::Explicit(initializer) => Some(initializer),
            MirStaticFieldInitialization::ZeroDefault => None,
        })
        .collect::<BTreeSet<_>>();
    if !initializers
        .keys()
        .all(|initializer| expected_initializers.contains(initializer))
    {
        program_error(
            errors,
            "final coordinator contains an initializer body without an explicit field",
        );
    }
}

fn verify_shutdown(
    view: LifecycleMirView<'_>,
    coordinator: &MirStaticLifecycleCoordinator,
    errors: &mut Vec<MirVerificationError>,
) {
    let order = view.lifecycle.plan().shutdown().collect::<Vec<_>>();
    if coordinator.shutdown().len() != order.len() {
        program_error(errors, "final destruction regions do not cover every field");
    }
    for (index, region) in coordinator.shutdown().iter().enumerate() {
        let Some(expected_field) = order.get(index) else {
            break;
        };
        if region.field != *expected_field {
            program_error(errors, "final destruction regions are not in reverse order");
            continue;
        }
        let Some(definition) = view.lifecycle.definition(*expected_field) else {
            continue;
        };
        if region.begin.field != *expected_field
            || region.begin.kind != MirStaticLifecycleTransitionKind::BeginDestruction
            || region.begin.span != definition.span
            || region.finish.field != *expected_field
            || region.finish.kind != MirStaticLifecycleTransitionKind::FinishDestruction
            || region.finish.span != definition.span
        {
            program_error(
                errors,
                format!("destruction transitions for {expected_field} are malformed"),
            );
        }
        let Some(expected_cleanup) = MirStaticValueCleanup::for_field(
            &view.program.optional_types,
            definition.ty,
            *expected_field,
            definition.span,
        ) else {
            program_error(
                errors,
                format!("destruction cleanup for {expected_field} has an unstorable type"),
            );
            continue;
        };
        if region.cleanup != expected_cleanup {
            program_error(
                errors,
                format!("destruction cleanup for {expected_field} is malformed"),
            );
        }
    }
}

fn verify_publication(
    initializer: &MirStaticInitializerBody,
    errors: &mut Vec<MirVerificationError>,
) {
    let publication = initializer.publication;
    let Some(exit) = initializer.block(publication.initialization_exit) else {
        program_error(errors, "final initializer publication exit is undeclared");
        return;
    };
    if !matches!(
        exit.terminator,
        Some(MirTerminator::Goto { target, .. }) if target == publication.cleanup_entry
    ) {
        program_error(
            errors,
            format!("initializer {} can bypass publication", initializer.id),
        );
    }
    if initializer.block(publication.cleanup_entry).is_none() {
        program_error(errors, "final initializer cleanup entry is undeclared");
        return;
    }

    let initialization = reachable_static_initializer_blocks(
        initializer,
        initializer.body.entry,
        Some((publication.initialization_exit, publication.cleanup_entry)),
    );
    let cleanup = reachable_static_initializer_blocks(initializer, publication.cleanup_entry, None);
    if !initialization.contains(&publication.initialization_exit)
        || initialization.iter().any(|block| cleanup.contains(block))
    {
        program_error(
            errors,
            format!(
                "initializer {} has overlapping or unreachable lifecycle regions",
                initializer.id
            ),
        );
    }
    let returns = initializer
        .body
        .blocks
        .iter()
        .filter(|block| {
            matches!(
                block.terminator,
                Some(
                    MirTerminator::Return { .. }
                        | MirTerminator::ReturnShared { .. }
                        | MirTerminator::ReturnOptionalShared { .. }
                )
            )
        })
        .map(|block| block.id)
        .collect::<BTreeSet<_>>();
    if returns.is_empty() || returns.iter().any(|block| !cleanup.contains(block)) {
        program_error(
            errors,
            format!("initializer {} can return before cleanup", initializer.id),
        );
    }
    if !destination_completed_on_every_publication_path(initializer) {
        program_error(
            errors,
            format!(
                "initializer {} does not complete its destination before publication",
                initializer.id
            ),
        );
    }
}
