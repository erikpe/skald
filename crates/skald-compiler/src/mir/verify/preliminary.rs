//! Structural verification for unplanned static lifecycle MIR.

use std::collections::{BTreeSet, VecDeque};

use crate::{
    identity::StaticInitializerId,
    mir::{
        MirArrayInstruction, MirDefinitionRef, MirInstruction, MirPlace, MirPlaceBase,
        MirStaticFieldInitialization, MirTerminator, MirType, PreliminaryMirProgram,
        PreliminaryMirStaticInitializer,
    },
};

use super::{context::Verifier, MirVerificationErrors};

pub(super) fn verify(program: &PreliminaryMirProgram) -> Result<(), MirVerificationErrors> {
    let fields = program.static_fields().copied().collect::<Vec<_>>();
    let mut verifier = Verifier::new_preliminary(program.program(), &fields);
    verifier.verify_program();
    verifier.verify_preliminary_static_fields(program);
    let errors = verifier.into_errors();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(MirVerificationErrors { errors })
    }
}

impl<'mir> Verifier<'mir> {
    fn verify_preliminary_static_fields(&mut self, preliminary: &'mir PreliminaryMirProgram) {
        let ordinary_fields = self
            .program
            .classes
            .iter()
            .flat_map(|class| &class.static_fields)
            .collect::<Vec<_>>();
        let fields = preliminary.static_fields().collect::<Vec<_>>();
        if fields.len() != ordinary_fields.len() {
            self.program_error("preliminary static-field inventory is incomplete");
        }

        let mut expected_initializers = BTreeSet::new();
        for (index, field) in fields.iter().enumerate() {
            let Some(ordinary) = ordinary_fields.get(index) else {
                break;
            };
            if field.field != ordinary.id || field.ty != ordinary.ty || field.span != ordinary.span
            {
                self.program_error(format!(
                    "preliminary static-field inventory entry {index} does not match {}",
                    ordinary.id
                ));
            }
            let expected_mode = field.initializer.map_or(
                MirStaticFieldInitialization::ZeroDefault,
                MirStaticFieldInitialization::Explicit,
            );
            if ordinary.initialization != expected_mode {
                self.program_error(format!(
                    "static field {} has initialization mode inconsistent with its preliminary inventory",
                    ordinary.id
                ));
            }
            if let Some(initializer) = field.initializer {
                if initializer != StaticInitializerId::from(field.field) {
                    self.program_error(format!(
                        "static field {} names mismatched initializer {initializer}",
                        field.field
                    ));
                }
                if !expected_initializers.insert(initializer) {
                    self.program_error(format!("duplicate static initializer {initializer}"));
                }
            }
        }

        let mut seen = BTreeSet::new();
        for initializer in preliminary.static_initializers() {
            if !seen.insert(initializer.id) {
                self.function_error(initializer.callable(), "duplicate static initializer body");
            }
            if initializer.id != StaticInitializerId::from(initializer.field) {
                self.function_error(
                    initializer.callable(),
                    "static initializer identity differs from its destination field",
                );
            }
            let Some(field) = self.program.static_field(initializer.field) else {
                self.function_error(
                    initializer.callable(),
                    "static initializer destination field is not declared",
                );
                continue;
            };
            if initializer.destination_type != field.ty {
                self.function_error(
                    initializer.callable(),
                    "static initializer destination type differs from its field",
                );
            }
            self.verify_definition(&[], MirType::Unit, MirDefinitionRef::from(initializer));
            self.verify_publication(initializer);
        }

        for missing in expected_initializers.difference(&seen) {
            self.function_error(*missing, "explicit static field has no initializer body");
        }
        for unexpected in seen.difference(&expected_initializers) {
            self.function_error(*unexpected, "initializer body has no explicit static field");
        }
    }

    fn verify_publication(&mut self, initializer: &PreliminaryMirStaticInitializer) {
        let publication = initializer.publication;
        let Some(exit) = initializer.block(publication.initialization_exit) else {
            self.function_error(
                initializer.callable(),
                "static publication names an undeclared initialization-exit block",
            );
            return;
        };
        if !matches!(
            exit.terminator,
            Some(MirTerminator::Goto { target, .. }) if target == publication.cleanup_entry
        ) {
            self.block_error(
                initializer.callable(),
                exit.id,
                "static publication must be one direct edge to cleanup",
            );
        }
        if initializer.block(publication.cleanup_entry).is_none() {
            self.function_error(
                initializer.callable(),
                "static publication names an undeclared cleanup-entry block",
            );
            return;
        }

        let initialization = reachable_static_initializer_blocks(
            initializer,
            initializer.body.entry,
            Some((publication.initialization_exit, publication.cleanup_entry)),
        );
        let cleanup =
            reachable_static_initializer_blocks(initializer, publication.cleanup_entry, None);
        if !initialization.contains(&publication.initialization_exit) {
            self.function_error(
                initializer.callable(),
                "static publication is unreachable from initializer entry",
            );
        }
        if initialization.iter().any(|block| cleanup.contains(block)) {
            self.function_error(
                initializer.callable(),
                "static initialization and post-publication cleanup regions overlap",
            );
        }

        if initialization
            .iter()
            .filter_map(|block| initializer.block(*block))
            .any(|block| {
                matches!(
                    block.terminator,
                    Some(
                        MirTerminator::Return { .. }
                            | MirTerminator::ReturnShared { .. }
                            | MirTerminator::ReturnOptionalShared { .. }
                    )
                )
            })
        {
            self.function_error(
                initializer.callable(),
                "static initializer returns before publication",
            );
        }
        if !destination_completed_on_every_publication_path(initializer) {
            self.function_error(
                initializer.callable(),
                "static initializer does not complete its destination on every publication path",
            );
        }
        for block in cleanup.iter().filter_map(|block| initializer.block(*block)) {
            if block
                .instructions
                .iter()
                .any(|instruction| initializes_static_field(instruction, initializer.field))
            {
                self.block_error(
                    initializer.callable(),
                    block.id,
                    "static destination is initialized after publication",
                );
            }
        }
    }
}

pub(crate) fn destination_completed_on_every_publication_path(
    initializer: &PreliminaryMirStaticInitializer,
) -> bool {
    let publication = initializer.publication;
    let mut seen = BTreeSet::new();
    let mut pending = VecDeque::from([(initializer.body.entry, false)]);
    let mut reached_publication = false;
    while let Some((block, mut completed)) = pending.pop_front() {
        if !seen.insert((block, completed)) {
            continue;
        }
        let Some(definition) = initializer.block(block) else {
            continue;
        };
        completed |= definition
            .instructions
            .iter()
            .any(|instruction| initializes_static_field(instruction, initializer.field));
        if block == publication.initialization_exit {
            reached_publication = true;
            if !completed {
                return false;
            }
        }
        let Some(terminator) = &definition.terminator else {
            continue;
        };
        for successor in terminator.successors() {
            if (block, successor) != (publication.initialization_exit, publication.cleanup_entry) {
                pending.push_back((successor, completed));
            }
        }
    }
    reached_publication
}

pub(crate) fn reachable_static_initializer_blocks(
    initializer: &PreliminaryMirStaticInitializer,
    entry: crate::mir::BlockId,
    excluded_edge: Option<(crate::mir::BlockId, crate::mir::BlockId)>,
) -> BTreeSet<crate::mir::BlockId> {
    let mut reached = BTreeSet::new();
    let mut pending = VecDeque::from([entry]);
    while let Some(block) = pending.pop_front() {
        if !reached.insert(block) {
            continue;
        }
        let Some(definition) = initializer.block(block) else {
            continue;
        };
        let Some(terminator) = &definition.terminator else {
            continue;
        };
        for successor in terminator.successors() {
            if excluded_edge == Some((block, successor)) {
                continue;
            }
            pending.push_back(successor);
        }
    }
    reached
}

pub(crate) fn initializes_static_field(
    instruction: &MirInstruction,
    field: crate::identity::StaticFieldId,
) -> bool {
    let exact = |place: &MirPlace| {
        place.base == MirPlaceBase::StaticLifecycleDestination(field)
            && place.projections.is_empty()
    };
    match instruction {
        MirInstruction::Store(operation) => exact(&operation.destination),
        MirInstruction::Call(operation) => operation.destination.as_ref().is_some_and(exact),
        MirInstruction::Initialize(operation) => exact(&operation.destination),
        MirInstruction::CopyConstruct(operation) => exact(&operation.destination),
        MirInstruction::StringInitialize(operation) => exact(&operation.destination),
        MirInstruction::OptionalInitialize(operation) => exact(&operation.destination),
        MirInstruction::ClassOptionalInitialize(operation) => exact(&operation.destination),
        MirInstruction::ClassOptionalPublish(operation) => exact(&operation.destination),
        MirInstruction::OptionalSharedInitialize(operation) => exact(&operation.destination),
        MirInstruction::SharedFieldInitialize(operation) => exact(&operation.destination),
        MirInstruction::Array(MirArrayInstruction::Adopt { destination, .. }) => exact(destination),
        _ => false,
    }
}
