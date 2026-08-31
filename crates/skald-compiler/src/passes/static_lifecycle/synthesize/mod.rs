//! Final target-independent static lifecycle coordinator synthesis.

use std::collections::BTreeMap;

use crate::mir::{
    MirProgram, MirProgramLifecycle, MirStaticActivationRegion, MirStaticDestructionRegion,
    MirStaticFieldInitialization, MirStaticLifecycleCoordinator, MirStaticValueCleanup,
};

use super::verify::{debug_assert_exact_synthesized_realization, VerifiedPlannedMirProgram};

/// Consumes an exactly verified planned product and moves its existing
/// initializer CFGs into one final program-owned lifecycle coordinator.
///
/// ```compile_fail
/// use skald_compiler::passes::static_lifecycle::{
///     synthesize_static_lifecycle, PlannedMirProgram,
/// };
/// let draft: PlannedMirProgram = todo!();
/// let _ = synthesize_static_lifecycle(draft);
/// ```
pub fn synthesize_static_lifecycle(verified: VerifiedPlannedMirProgram) -> MirProgram {
    let planned = verified.into_program();
    let (preliminary, planned_lifecycle) = planned.into_executable_parts();
    let (mut program, _fields, initializers) = preliminary.into_parts();
    let mut initializers = initializers
        .into_iter()
        .map(|initializer| (initializer.id, initializer))
        .collect::<BTreeMap<_, _>>();
    let mut activation = Vec::with_capacity(planned_lifecycle.plan().activation().len());
    let mut ordered_initializers = Vec::with_capacity(initializers.len());
    for field in planned_lifecycle.plan().activation() {
        let definition = *planned_lifecycle
            .definition(*field)
            .expect("verified activation field must have a definition");
        let region = match definition.initialization {
            MirStaticFieldInitialization::ZeroDefault => {
                MirStaticActivationRegion::zero_default(*field, definition.span)
            }
            MirStaticFieldInitialization::Explicit(id) => {
                let initializer = initializers
                    .remove(&id)
                    .expect("verified explicit activation has one initializer body");
                let region = MirStaticActivationRegion::explicit(
                    *field,
                    id,
                    initializer.span,
                    initializer.publication.span,
                );
                ordered_initializers.push(initializer);
                region
            }
        };
        activation.push(region);
    }
    debug_assert!(initializers.values().all(|initializer| !planned_lifecycle
        .proof()
        .activation()
        .contains(initializer.field)));

    let shutdown = planned_lifecycle
        .plan()
        .shutdown()
        .map(|field| {
            let definition = *planned_lifecycle
                .definition(field)
                .expect("verified shutdown field must have a definition");
            let cleanup = MirStaticValueCleanup::for_field(
                &program.optional_types,
                definition.ty,
                field,
                definition.span,
            )
            .expect("verified static lifecycle definitions have storable types");
            MirStaticDestructionRegion::new(field, definition.span, cleanup)
        })
        .collect();

    let lifecycle = MirProgramLifecycle::new(planned_lifecycle);

    program.static_lifecycle = Some(MirStaticLifecycleCoordinator::new(
        lifecycle,
        ordered_initializers,
        activation,
        shutdown,
    ));
    debug_assert_exact_synthesized_realization(&program);
    program
}

#[cfg(test)]
mod tests;
