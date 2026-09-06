use std::collections::HashSet;

use super::{
    super::{
        super::model::{
            MirArrayAssignElement, MirArrayCopyElement, MirArrayDefaultElement,
            MirArrayDestroyElement, MirArrayInstruction, MirBasicBlock, MirDefinitionRef,
            MirInstruction, MirPlace, MirTerminator, MirType, ValueId,
        },
        context::Verifier,
    },
    indexed_element_requires_advance, IndexedArrayLoopShape,
};

impl Verifier<'_> {
    pub(in crate::mir::verify) fn indexed_array_loop_shape_is_canonical(
        &self,
        function: MirDefinitionRef<'_>,
        shape: IndexedArrayLoopShape,
    ) -> bool {
        let IndexedArrayLoopShape {
            header,
            backing,
            prefix,
            length,
            binding,
            body_target,
            complete_target,
        } = shape;
        let mut begin_blocks = Vec::new();
        let mut binding_count = 0;
        let mut initialization_count = 0;
        let mut advance_count = 0;
        let mut end_blocks = Vec::new();
        let mut completion_count = 0;

        for block in &function.body().blocks {
            for (index, instruction) in block.instructions.iter().enumerate() {
                match instruction {
                    MirInstruction::Array(MirArrayInstruction::BeginIndexed {
                        backing: operation_backing,
                        prefix: operation_prefix,
                        length: operation_length,
                        ..
                    }) if (*operation_backing, *operation_prefix, *operation_length)
                        == (backing, prefix, length) =>
                    {
                        begin_blocks.push(block.id)
                    }
                    MirInstruction::Array(MirArrayInstruction::BindIndexed {
                        backing: operation_backing,
                        prefix: operation_prefix,
                        length: operation_length,
                        binding: operation_binding,
                        ..
                    }) if (
                        *operation_backing,
                        *operation_prefix,
                        *operation_length,
                        *operation_binding,
                    ) == (backing, prefix, length, binding) =>
                    {
                        binding_count += 1
                    }
                    MirInstruction::Array(MirArrayInstruction::InitializeIndexedElement {
                        backing: operation_backing,
                        prefix: operation_prefix,
                        ..
                    }) if (*operation_backing, *operation_prefix) == (backing, prefix) => {
                        initialization_count += 1;
                    }
                    MirInstruction::Array(MirArrayInstruction::AdvanceIndexedElement {
                        backing: operation_backing,
                        prefix: operation_prefix,
                        ..
                    }) if (*operation_backing, *operation_prefix) == (backing, prefix) => {
                        advance_count += 1;
                    }
                    MirInstruction::Array(MirArrayInstruction::EndIndexedElement {
                        backing: operation_backing,
                        prefix: operation_prefix,
                        length: operation_length,
                        ..
                    }) if (*operation_backing, *operation_prefix, *operation_length)
                        == (backing, prefix, length) =>
                    {
                        if index > 0
                            && matches!(
                                &block.instructions[index - 1],
                                MirInstruction::StorageDead(dead) if dead.storage == binding
                            )
                            && index + 1 == block.instructions.len()
                            && matches!(block.terminator, Some(MirTerminator::Goto { target, .. }) if target == header)
                        {
                            end_blocks.push(block.id);
                        }
                    }
                    MirInstruction::Array(MirArrayInstruction::CompleteIndexed {
                        backing: operation_backing,
                        prefix: operation_prefix,
                        length: operation_length,
                        ..
                    }) if (*operation_backing, *operation_prefix, *operation_length)
                        == (backing, prefix, length) =>
                    {
                        completion_count += 1
                    }
                    _ => {}
                }
            }
        }

        let begin_is_canonical = matches!(begin_blocks.as_slice(), [begin]
        if function.block(*begin).is_some_and(|block| matches!(
            block.terminator,
            Some(MirTerminator::Goto { target, .. }) if target == header
        )));
        let allocation_establishes_length = matches!(begin_blocks.as_slice(), [begin]
        if function.body().blocks.iter().filter(|block| matches!(
            block.terminator,
            Some(MirTerminator::ArrayOperationCheck {
                failure: crate::mir::MirArrayFailure::AllocationSize,
                success_target,
                ..
            }) if success_target == *begin
        )).count() == 1
        && function.body().blocks.iter().any(|block| {
            if !matches!(
                block.terminator,
                Some(MirTerminator::ArrayOperationCheck {
                    failure: crate::mir::MirArrayFailure::AllocationSize,
                    success_target,
                    ..
                }) if success_target == *begin
            ) {
                return false;
            }
            let Some(MirInstruction::Array(MirArrayInstruction::Allocate {
                backing: allocated_backing,
                length: allocated_length,
                ..
            })) = block.instructions.last()
            else {
                return false;
            };
            *allocated_backing == backing
                && block.instructions[..block.instructions.len() - 1]
                    .iter()
                    .filter(|instruction| matches!(
                        instruction,
                        MirInstruction::Store(store)
                            if store.destination == MirPlace::base(length)
                                && store.value == *allocated_length
                    ))
                    .count()
                    == 1
        }));
        let body_begins_epoch = function.block(body_target).is_some_and(|body| {
            matches!(
                body.instructions.as_slice(),
                [
                    MirInstruction::StorageLive(live),
                    MirInstruction::Array(MirArrayInstruction::BindIndexed {
                        backing: body_backing,
                        prefix: body_prefix,
                        length: body_length,
                        binding: body_binding,
                        ..
                    }),
                    ..
                ] if live.storage == binding
                    && *body_backing == backing
                    && *body_prefix == prefix
                    && *body_length == length
                    && *body_binding == binding
            )
        });
        let completion_is_canonical = function.block(complete_target).is_some_and(|complete| {
            matches!(
                complete.instructions.first(),
                Some(MirInstruction::Array(MirArrayInstruction::CompleteIndexed {
                    backing: complete_backing,
                    prefix: complete_prefix,
                    length: complete_length,
                    ..
                })) if *complete_backing == backing
                    && *complete_prefix == prefix
                    && *complete_length == length
            )
        });
        let element_transition_is_canonical = function
            .storage(backing)
            .and_then(|storage| match storage.ty {
                MirType::Array(array) => self.program.array_type(array),
                _ => None,
            })
            .is_some_and(|array| {
                if array.element.is_scalar_value() {
                    initialization_count == 1 && advance_count == 0
                } else if indexed_element_requires_advance(array.element) {
                    initialization_count == 0 && advance_count == 1
                } else {
                    false
                }
            });
        let exclusive_targets = function.body().blocks.iter().all(|block| {
            block.id == header
                || block.terminator.as_ref().is_none_or(|terminator| {
                    !terminator
                        .successors()
                        .any(|target| target == body_target || target == complete_target)
                })
        });

        begin_is_canonical
            && allocation_establishes_length
            && body_begins_epoch
            && binding_count == 1
            && element_transition_is_canonical
            && matches!(end_blocks.as_slice(), [_])
            && completion_count == 1
            && completion_is_canonical
            && exclusive_targets
    }

    pub(in crate::mir::verify) fn verify_array_declarations(&mut self) {
        let arrays: Vec<_> = self.program.array_types.iter().cloned().collect();
        let mut seen = HashSet::new();
        for (index, array) in arrays.iter().enumerate() {
            if array.id.index() != index {
                self.program_error(format!(
                    "array type table index {index} contains {}",
                    array.id
                ));
            }
            if !seen.insert(array.id) {
                self.program_error(format!("duplicate array type {}", array.id));
            }
            self.verify_array_referenced_type(array.element, "element");
            if let Some(operation) = array.lifecycle.default {
                self.verify_array_default(array.element, operation);
            }
            if let Some(operation) = array.lifecycle.copy {
                self.verify_array_copy(operation);
            }
            if let Some(operation) = array.lifecycle.assignment {
                self.verify_array_assignment(operation);
            }
            self.verify_array_destruction(array.lifecycle.destruction);
            let scalar = array.element.is_scalar_value();
            if matches!(
                array.lifecycle.default,
                Some(MirArrayDefaultElement::Primitive)
            ) != scalar
                || matches!(array.lifecycle.copy, Some(MirArrayCopyElement::Primitive)) != scalar
                || matches!(
                    array.lifecycle.assignment,
                    Some(MirArrayAssignElement::Primitive)
                ) != scalar
                || (scalar && array.lifecycle.destruction != MirArrayDestroyElement::Trivial)
            {
                self.program_error(format!(
                    "array {} lifecycle is incompatible with element type {}",
                    array.id, array.element
                ));
            }
        }
    }

    fn verify_array_referenced_type(&mut self, ty: MirType, role: &str) {
        let declared = match ty {
            MirType::Class(class) => self.program.class(class).is_some(),
            MirType::Optional(optional) => self.program.optional_type(optional).is_some(),
            MirType::Array(array) => self.program.array_type(array).is_some(),
            MirType::Shared(target) => match target {
                crate::mir::MirSharedTarget::Class(class) => self.program.class(class).is_some(),
                crate::mir::MirSharedTarget::Interface(interface) => {
                    self.program.interface(interface).is_some()
                }
                crate::mir::MirSharedTarget::Array(array) => {
                    self.program.array_type(array).is_some()
                }
                crate::mir::MirSharedTarget::Obj => true,
                crate::mir::MirSharedTarget::OptionalBox(target) => {
                    self.program.optional_box_type(target).is_some()
                }
            },
            MirType::Interface(_) | MirType::Obj | MirType::Unit => false,
            _ => true,
        };
        if !declared {
            self.program_error(format!(
                "array {role} type {ty} is not a declared storable type"
            ));
        }
    }

    fn verify_array_default(&mut self, element: MirType, operation: MirArrayDefaultElement) {
        match operation {
            MirArrayDefaultElement::Class { class, initializer }
            | MirArrayDefaultElement::SharedClass { class, initializer } => {
                if self.program.initializer(initializer).is_none() || initializer.class() != class {
                    self.program_error("array default element names an invalid initializer");
                }
            }
            MirArrayDefaultElement::ArrayEmpty(array)
            | MirArrayDefaultElement::SharedArrayEmpty(array) => {
                if self.program.array_type(array).is_none() {
                    self.program_error("array default element names an undeclared nested array");
                }
            }
            MirArrayDefaultElement::SharedOptionalBoxAbsent(target) => {
                if element != MirType::Shared(crate::mir::MirSharedTarget::OptionalBox(target)) {
                    self.program_error(
                        "array optional-box default does not match its declared element type",
                    );
                } else if !self
                    .program
                    .optional_box_type(target)
                    .is_some_and(|metadata| metadata.exact_optional.is_some())
                {
                    self.program_error(
                        "array default element names a non-constructible optional box",
                    );
                }
            }
            MirArrayDefaultElement::Primitive | MirArrayDefaultElement::OptionalAbsent => {}
        }
    }

    fn verify_array_copy(&mut self, operation: MirArrayCopyElement) {
        match operation {
            MirArrayCopyElement::Class { class, operation }
            | MirArrayCopyElement::OptionalClass { class, operation } => {
                if self
                    .program
                    .class(class)
                    .and_then(|class| class.copy_constructor.selected())
                    != Some(operation)
                {
                    self.program_error("array copy element has an invalid class copy operation");
                }
            }
            MirArrayCopyElement::Array(array) => {
                if self.program.array_type(array).is_none() {
                    self.program_error("array copy element names an undeclared nested array");
                }
            }
            MirArrayCopyElement::Shared(target) | MirArrayCopyElement::OptionalShared(target) => {
                self.verify_array_referenced_type(MirType::Shared(target), "copy operation")
            }
            MirArrayCopyElement::Primitive | MirArrayCopyElement::OptionalPrimitive => {}
            MirArrayCopyElement::Optional(optional) => {
                if !self
                    .program
                    .optional_type(optional)
                    .is_some_and(|metadata| {
                        matches!(
                            metadata.storage,
                            crate::mir::MirOptionalStorage::Nested(_)
                                | crate::mir::MirOptionalStorage::InlineArray(_)
                        )
                    })
                {
                    self.program_error("array copy names an invalid nested optional");
                }
            }
        }
    }

    fn verify_array_assignment(&mut self, operation: MirArrayAssignElement) {
        match operation {
            MirArrayAssignElement::Class { class, operation } => {
                if self
                    .program
                    .class(class)
                    .and_then(|class| class.copy_assignment.selected())
                    != Some(operation)
                {
                    self.program_error(
                        "array assignment element has an invalid class assignment operation",
                    );
                }
            }
            MirArrayAssignElement::OptionalClass {
                class,
                copy_constructor,
                copy_assignment,
            } => {
                let declaration = self.program.class(class);
                if declaration.and_then(|class| class.copy_constructor.selected())
                    != Some(copy_constructor)
                    || declaration.and_then(|class| class.copy_assignment.selected())
                        != Some(copy_assignment)
                {
                    self.program_error(
                        "array optional assignment element has invalid copy operations",
                    );
                }
            }
            MirArrayAssignElement::Array(array) => {
                if self.program.array_type(array).is_none() {
                    self.program_error("array assignment names an undeclared nested array");
                }
            }
            MirArrayAssignElement::Shared(target)
            | MirArrayAssignElement::OptionalShared(target) => {
                self.verify_array_referenced_type(MirType::Shared(target), "assignment operation")
            }
            MirArrayAssignElement::Primitive | MirArrayAssignElement::OptionalPrimitive => {}
            MirArrayAssignElement::Optional(optional) => {
                if !self
                    .program
                    .optional_type(optional)
                    .is_some_and(|metadata| {
                        matches!(
                            metadata.storage,
                            crate::mir::MirOptionalStorage::Nested(_)
                                | crate::mir::MirOptionalStorage::InlineArray(_)
                        )
                    })
                {
                    self.program_error("array assignment names an invalid nested optional");
                }
            }
        }
    }

    fn verify_array_destruction(&mut self, operation: MirArrayDestroyElement) {
        match operation {
            MirArrayDestroyElement::Class(class) | MirArrayDestroyElement::OptionalClass(class) => {
                if self.program.class(class).is_none() {
                    self.program_error("array destruction names an undeclared class");
                }
            }
            MirArrayDestroyElement::Array(array) => {
                if self.program.array_type(array).is_none() {
                    self.program_error("array destruction names an undeclared nested array");
                }
            }
            MirArrayDestroyElement::Shared(target)
            | MirArrayDestroyElement::OptionalShared(target) => {
                self.verify_array_referenced_type(MirType::Shared(target), "destruction operation")
            }
            MirArrayDestroyElement::Trivial => {}
            MirArrayDestroyElement::Optional(optional) => {
                if !self
                    .program
                    .optional_type(optional)
                    .is_some_and(|metadata| {
                        matches!(
                            metadata.storage,
                            crate::mir::MirOptionalStorage::Nested(_)
                                | crate::mir::MirOptionalStorage::InlineArray(_)
                        )
                    })
                {
                    self.program_error("array destruction names an invalid nested optional");
                }
            }
        }
    }

    pub(in crate::mir::verify) fn verify_array_instruction(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        instruction: &MirArrayInstruction,
        defined: &HashSet<ValueId>,
    ) {
        self.verify_array_instruction_storage(function, block, instruction, defined);
        self.verify_array_projection_instruction(function, block, instruction, defined);
        self.verify_array_anchor_instruction(function, block, instruction);
    }
}
