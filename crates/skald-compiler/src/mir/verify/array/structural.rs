use std::collections::HashSet;

use super::super::{
    super::model::{
        MirArrayAssignElement, MirArrayCopyElement, MirArrayDefaultElement, MirArrayDestroyElement,
        MirArrayInstruction, MirBasicBlock, MirDefinitionRef, MirType, ValueId,
    },
    context::Verifier,
};

impl Verifier<'_> {
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
                self.verify_array_default(operation);
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

    fn verify_array_default(&mut self, operation: MirArrayDefaultElement) {
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
