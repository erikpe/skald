//! Final target-independent static lifecycle coordinator synthesis.

use std::collections::BTreeMap;

use crate::mir::{
    MirProgram, MirStaticActivationRegion, MirStaticActivationWork, MirStaticDestructionRegion,
    MirStaticFieldInitialization, MirStaticLifecycleCoordinator, MirStaticValueCleanup,
    MirVerificationErrors, PlannedMirProgram,
};

use super::{
    verify::{debug_assert_exact_synthesized_realization, verify_planned_mir},
    verify_synthesized_mir,
};

/// Consumes a verified planned product and moves its existing initializer CFGs
/// into one final program-owned lifecycle coordinator.
pub fn synthesize_static_lifecycle(
    planned: PlannedMirProgram,
) -> Result<MirProgram, MirVerificationErrors> {
    verify_planned_mir(&planned)?;

    let (preliminary, lifecycle) = planned.into_parts();
    let (mut program, _fields, initializers) = preliminary.into_parts();
    let mut initializers = initializers
        .into_iter()
        .map(|initializer| (initializer.id, initializer))
        .collect::<BTreeMap<_, _>>();
    let definitions = lifecycle
        .definitions()
        .iter()
        .map(|definition| (definition.field, *definition))
        .collect::<BTreeMap<_, _>>();

    let mut transitions = lifecycle.activation().iter().copied();
    let mut activation = Vec::with_capacity(lifecycle.plan().activation().len());
    let mut ordered_initializers = Vec::with_capacity(initializers.len());
    for field in lifecycle.plan().activation() {
        let definition = definitions[field];
        let (work, region_transitions) = match definition.initialization {
            MirStaticFieldInitialization::ZeroDefault => (
                MirStaticActivationWork::ZeroDefault,
                vec![transitions
                    .next()
                    .expect("verified zero-default activation has one transition")],
            ),
            MirStaticFieldInitialization::Explicit(id) => {
                ordered_initializers.push(
                    initializers
                        .remove(&id)
                        .expect("verified explicit activation has one initializer body"),
                );
                (
                    MirStaticActivationWork::Explicit(id),
                    vec![
                        transitions
                            .next()
                            .expect("verified explicit activation has a begin transition"),
                        transitions
                            .next()
                            .expect("verified explicit activation has a publish transition"),
                    ],
                )
            }
        };
        activation.push(MirStaticActivationRegion {
            field: *field,
            work,
            transitions: region_transitions,
        });
    }
    debug_assert!(transitions.next().is_none());
    debug_assert!(initializers.is_empty());

    let shutdown_transitions = lifecycle.shutdown();
    let shutdown = lifecycle
        .plan()
        .shutdown()
        .iter()
        .enumerate()
        .map(|(index, field)| {
            let definition = definitions[field];
            MirStaticDestructionRegion {
                field: *field,
                begin: shutdown_transitions[index * 2],
                cleanup: MirStaticValueCleanup::for_field(
                    &program.optional_types,
                    definition.ty,
                    *field,
                    definition.span,
                )
                .expect("verified static lifecycle definitions have storable types"),
                finish: shutdown_transitions[index * 2 + 1],
            }
        })
        .collect();

    program.static_lifecycle = Some(MirStaticLifecycleCoordinator::new(
        lifecycle,
        ordered_initializers,
        activation,
        shutdown,
    ));
    debug_assert_exact_synthesized_realization(&program);
    verify_synthesized_mir(&program)?;
    Ok(program)
}

#[cfg(test)]
mod tests;
