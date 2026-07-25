//! Program-wide MIR declaration and definition-table verification.
//!
//! This module owns deterministic metadata traversal. It delegates executable
//! bodies through one focused verifier method and contains no block or
//! instruction verification.

use std::collections::HashSet;

use crate::{identity::CallableId, lexical_policy::is_source_identifier};

use super::{
    super::model::{
        MirClassDeclaration, MirCopyCapability, MirDestructionStep, MirFunctionLinkage,
        MirParameter, MirParameterMode, MirReceiverAccess, MirSynthesizedCopy,
        MirSynthesizedFieldCopy, MirType,
    },
    context::Verifier,
};

impl<'mir> Verifier<'mir> {
    pub(super) fn verify_program(&mut self) {
        self.verify_classes();
        self.verify_virtual_families();
        self.verify_interfaces();
        let entry_declaration = self.program.declarations.get(self.program.entry_function);
        if entry_declaration.is_none() {
            self.program_error(format!(
                "entry function {} is not declared",
                self.program.entry_function
            ));
        } else {
            if !matches!(
                entry_declaration.map(|declaration| &declaration.linkage),
                Some(MirFunctionLinkage::Internal)
            ) {
                self.program_error("entry function must have internal linkage");
            }
            if self
                .program
                .definitions
                .get(self.program.entry_function)
                .is_none()
            {
                self.program_error(format!(
                    "entry function {} has no definition",
                    self.program.entry_function
                ));
            }
            if entry_declaration.is_some_and(|declaration| {
                !declaration.parameters.is_empty() || declaration.return_type != MirType::I64
            }) {
                self.program_error("entry function must have signature `fn main() -> i64`");
            }
        }

        let declarations: Vec<_> = self.program.declarations.iter().collect();
        let mut seen = HashSet::new();
        for (index, declaration) in declarations.iter().enumerate() {
            if declaration.id.index() != index {
                self.function_error(
                    declaration.id,
                    format!(
                        "function declaration table index {index} contains {}",
                        declaration.id
                    ),
                );
            }
            if !seen.insert(declaration.id) {
                self.function_error(declaration.id, "duplicate function declaration ID");
            }
            self.verify_parameters_declaration(
                &format!("function {}", declaration.id),
                &declaration.parameters,
            );
            if let MirType::Class(class) = declaration.return_type {
                if self.program.class(class).is_none() {
                    self.function_error(
                        declaration.id,
                        format!("function result has undeclared class type {class}"),
                    );
                }
            }
            if let MirType::Interface(interface) = declaration.return_type {
                if self.program.interface(interface).is_none() {
                    self.function_error(
                        declaration.id,
                        format!("function result has undeclared interface type {interface}"),
                    );
                }
            }
            if matches!(
                declaration.return_type,
                MirType::Interface(_) | MirType::Obj
            ) {
                self.function_error(
                    declaration.id,
                    "function result cannot have a non-owning interface or `Obj` type",
                );
            }
            if let MirFunctionLinkage::External { symbol } = &declaration.linkage {
                if declaration.parameters.iter().any(|parameter| {
                    parameter.mode != MirParameterMode::Value
                        || matches!(
                            parameter.ty,
                            MirType::Class(_)
                                | MirType::Interface(_)
                                | MirType::Obj
                                | MirType::Shared(_)
                                | MirType::OptionalPrimitive(_)
                                | MirType::OptionalClass(_)
                        )
                }) {
                    self.function_error(
                        declaration.id,
                        "external function cannot declare alias, object value, or shared-owner parameters",
                    );
                }
                if matches!(
                    declaration.return_type,
                    MirType::Class(_)
                        | MirType::Interface(_)
                        | MirType::Obj
                        | MirType::Shared(_)
                        | MirType::OptionalPrimitive(_)
                        | MirType::OptionalClass(_)
                ) {
                    self.function_error(
                        declaration.id,
                        "external function cannot return an object value or shared owner",
                    );
                }
                if symbol != &declaration.name || !is_source_identifier(symbol) {
                    self.function_error(
                        declaration.id,
                        "external symbol must be the declaration's exact source identifier",
                    );
                }
            }
        }

        let mut defined_functions = HashSet::new();
        for (index, definition) in self.program.definitions.indexed_slots() {
            let Some(definition) = definition else {
                continue;
            };
            if definition.function.index() != index {
                self.function_error(
                    definition.function,
                    format!(
                        "function definition table index {index} contains {}",
                        definition.function
                    ),
                );
            }
            if !defined_functions.insert(definition.function) {
                self.function_error(definition.function, "duplicate function definition");
            }
            let Some(declaration) = self.program.declarations.get(definition.function) else {
                self.function_error(
                    definition.function,
                    "function definition has no declaration",
                );
                continue;
            };
            if !matches!(declaration.linkage, MirFunctionLinkage::Internal) {
                self.function_error(
                    definition.function,
                    "external function must not have a Skald definition",
                );
            }
            self.verify_definition(
                &declaration.parameters,
                declaration.return_type,
                definition.into(),
            );
        }

        for declaration in declarations {
            match (
                &declaration.linkage,
                self.program.definitions.get(declaration.id),
            ) {
                (MirFunctionLinkage::Internal, None) => {
                    self.function_error(declaration.id, "internal function has no definition");
                }
                (MirFunctionLinkage::External { .. }, Some(_)) => {
                    // Reported while walking definition slots above.
                }
                _ => {}
            }
        }

        for (table_key, definition) in self.program.member_definitions.indexed_entries() {
            let callable = definition.callable;
            if table_key != callable {
                self.function_error(
                    callable,
                    format!("member definition table key {table_key} contains {callable}"),
                );
            }
            let signature = match callable {
                CallableId::Initializer(initializer) => self
                    .program
                    .initializer(initializer)
                    .map(|declaration| (&declaration.parameters[..], MirType::Unit)),
                CallableId::CopyConstructor(copy) => self
                    .program
                    .copy_constructor(copy)
                    .map(|declaration| (&declaration.parameters[..], MirType::Unit)),
                CallableId::CopyAssignment(assignment) => {
                    self.program.copy_assignment(assignment).map(|declaration| {
                        (std::slice::from_ref(&declaration.parameter), MirType::Unit)
                    })
                }
                CallableId::Destructor(destructor) => self
                    .program
                    .destructor(destructor)
                    .map(|_| (&[][..], MirType::Unit)),
                CallableId::Method(method) => self
                    .program
                    .method(method)
                    .map(|declaration| (&declaration.parameters[..], declaration.return_type)),
                CallableId::Function(_) => None,
            };
            let Some((parameters, return_type)) = signature else {
                self.function_error(callable, "member definition has no matching declaration");
                continue;
            };
            self.verify_definition(parameters, return_type, definition.into());
        }
    }

    fn verify_classes(&mut self) {
        self.verify_class_hierarchy();
        for (class_index, class) in self.program.classes.iter().enumerate() {
            if class.id.index() != class_index {
                self.program_error(format!(
                    "class declaration table index {class_index} contains {}",
                    class.id
                ));
            }
            for (index, field) in class.fields.iter().enumerate() {
                if field.id.class() != class.id || field.id.index() != index {
                    self.program_error(format!(
                        "class {} field table index {index} contains {}",
                        class.id, field.id
                    ));
                }
                match field.ty {
                    MirType::Interface(_) | MirType::Obj => self.program_error(format!(
                        "field {} cannot have a non-owning interface or `Obj` type",
                        field.id
                    )),
                    MirType::Unit => self.program_error(format!(
                        "field {} cannot have payload-free type `unit`",
                        field.id
                    )),
                    MirType::Class(target) if self.program.class(target).is_none() => {
                        self.program_error(format!(
                            "field {} has undeclared class type {target}",
                            field.id
                        ));
                    }
                    _ => {}
                }
            }
            for (index, initializer) in class.initializers.iter().enumerate() {
                if initializer.id.class() != class.id || initializer.id.index() != index {
                    self.program_error(format!(
                        "class {} initializer table index {index} contains {}",
                        class.id, initializer.id
                    ));
                }
                self.verify_member_parameters(
                    &format!("initializer {}", initializer.id),
                    &initializer.parameters,
                );
                if self
                    .program
                    .member_definition(initializer.id.into())
                    .is_none()
                {
                    self.program_error(format!(
                        "initializer {} has no member definition",
                        initializer.id
                    ));
                }
            }
            self.verify_copy_constructor_metadata(class);
            self.verify_copy_assignment_metadata(class);
            if let Some(destructor) = &class.destruction.destructor {
                if destructor.id.class() != class.id || destructor.id.index() != 0 {
                    self.program_error(format!(
                        "class {} destructor declaration contains {}",
                        class.id, destructor.id
                    ));
                }
                if destructor.receiver_access != MirReceiverAccess::Mutable {
                    self.program_error(format!(
                        "destructor {} must have mutable receiver access",
                        destructor.id
                    ));
                }
                if self
                    .program
                    .member_definition(destructor.id.into())
                    .is_none()
                {
                    self.program_error(format!(
                        "destructor {} has no member definition",
                        destructor.id
                    ));
                }
            }
            let mut expected_steps = Vec::new();
            if let Some(destructor) = &class.destruction.destructor {
                expected_steps.push(MirDestructionStep::UserBody(destructor.id));
            }
            expected_steps.extend(
                class
                    .fields
                    .iter()
                    .rev()
                    .filter_map(|field| match field.ty {
                        MirType::Class(_) => Some(MirDestructionStep::Field(field.id)),
                        MirType::Shared(_) => Some(MirDestructionStep::SharedField(field.id)),
                        MirType::OptionalShared(_) => {
                            Some(MirDestructionStep::OptionalSharedField(field.id))
                        }
                        MirType::OptionalClass(_) => {
                            Some(MirDestructionStep::OptionalClassField(field.id))
                        }
                        _ => None,
                    }),
            );
            if let Some(base) = class.direct_base {
                expected_steps.push(MirDestructionStep::Base(base.class));
            }
            if class.destruction.steps != expected_steps {
                self.program_error(format!(
                    "class {} destruction plan must run its user body first and owning fields in reverse declaration order, then its direct base",
                    class.id
                ));
            }
            for (index, method) in class.methods.iter().enumerate() {
                if method.id.class() != class.id || method.id.index() != index {
                    self.program_error(format!(
                        "class {} method table index {index} contains {}",
                        class.id, method.id
                    ));
                }
                self.verify_member_parameters(&format!("method {}", method.id), &method.parameters);
                if let MirType::Class(class) = method.return_type {
                    if self.program.class(class).is_none() {
                        self.program_error(format!(
                            "method {} has undeclared result class {class}",
                            method.id
                        ));
                    }
                }
                if let MirType::Interface(interface) = method.return_type {
                    if self.program.interface(interface).is_none() {
                        self.program_error(format!(
                            "method {} has undeclared result interface {interface}",
                            method.id
                        ));
                    }
                }
                if matches!(method.return_type, MirType::Interface(_) | MirType::Obj) {
                    self.program_error(format!(
                        "method {} cannot return a non-owning interface or `Obj` type",
                        method.id
                    ));
                }
            }
        }
    }

    fn verify_copy_constructor_metadata(&mut self, class: &MirClassDeclaration) {
        if let Some(declaration) = &class.copy_constructor_declaration {
            if declaration.id.class() != class.id || declaration.id.index() != 0 {
                self.program_error(format!(
                    "class {} copy-constructor declaration contains {}",
                    class.id, declaration.id
                ));
            }
            if declaration.parameters != [MirParameter::read_only_alias(MirType::Class(class.id))] {
                self.program_error(format!(
                    "copy constructor {} must take one read-only exact-class alias",
                    declaration.id
                ));
            }
        }
        match &class.copy_constructor {
            MirCopyCapability::User(copy) => {
                if class
                    .copy_constructor_declaration
                    .as_ref()
                    .map(|item| item.id)
                    != Some(copy.operation)
                {
                    self.program_error(format!(
                        "class {} user copy-constructor capability has no matching declaration",
                        class.id
                    ));
                }
                if self
                    .program
                    .member_definition(copy.operation.into())
                    .is_none()
                {
                    self.program_error(format!(
                        "copy constructor {} has no member definition",
                        copy.operation
                    ));
                }
                self.verify_constructor_base(class, copy.base);
            }
            MirCopyCapability::Synthesized(copy) => {
                if class.copy_constructor_declaration.is_some() {
                    self.program_error(format!(
                        "class {} synthesized copy constructor must not have a user declaration",
                        class.id
                    ));
                }
                self.verify_synthesized_constructor(class, copy);
            }
            MirCopyCapability::Unavailable => {
                if class.copy_constructor_declaration.is_some() {
                    self.program_error(format!(
                        "class {} unavailable copy constructor must not have a user declaration",
                        class.id
                    ));
                }
            }
        }
    }

    fn verify_copy_assignment_metadata(&mut self, class: &MirClassDeclaration) {
        if let Some(declaration) = &class.copy_assignment_declaration {
            if declaration.id.class() != class.id || declaration.id.index() != 0 {
                self.program_error(format!(
                    "class {} copy-assignment declaration contains {}",
                    class.id, declaration.id
                ));
            }
            if declaration.parameter != MirParameter::read_only_alias(MirType::Class(class.id)) {
                self.program_error(format!(
                    "copy assignment {} must take one read-only exact-class alias",
                    declaration.id
                ));
            }
        }
        match &class.copy_assignment {
            MirCopyCapability::User(copy) => {
                if class
                    .copy_assignment_declaration
                    .as_ref()
                    .map(|item| item.id)
                    != Some(copy.operation)
                {
                    self.program_error(format!(
                        "class {} user copy-assignment capability has no matching declaration",
                        class.id
                    ));
                }
                if self
                    .program
                    .member_definition(copy.operation.into())
                    .is_none()
                {
                    self.program_error(format!(
                        "copy assignment {} has no member definition",
                        copy.operation
                    ));
                }
                self.verify_assignment_base(class, copy.base);
            }
            MirCopyCapability::Synthesized(copy) => {
                if class.copy_assignment_declaration.is_some() {
                    self.program_error(format!(
                        "class {} synthesized copy assignment must not have a user declaration",
                        class.id
                    ));
                }
                self.verify_synthesized_assignment(class, copy);
            }
            MirCopyCapability::Unavailable => {
                if class.copy_assignment_declaration.is_some() {
                    self.program_error(format!(
                        "class {} unavailable copy assignment must not have a user declaration",
                        class.id
                    ));
                }
            }
        }
    }

    fn verify_synthesized_constructor(
        &mut self,
        class: &MirClassDeclaration,
        copy: &MirSynthesizedCopy<crate::identity::CopyConstructorId>,
    ) {
        self.verify_constructor_base(class, copy.base);
        if copy.class != class.id || copy.fields.len() != class.fields.len() {
            self.program_error(format!(
                "class {} synthesized copy-construction plan has the wrong owner or field count",
                class.id
            ));
            return;
        }
        for (field, step) in class.fields.iter().zip(&copy.fields) {
            let valid = match (field.ty, step) {
                (
                    MirType::Class(target),
                    MirSynthesizedFieldCopy::Class {
                        field: id,
                        operation,
                    },
                ) => {
                    *id == field.id
                        && self
                            .program
                            .class(target)
                            .and_then(|class| class.copy_constructor.selected())
                            == Some(*operation)
                }
                (ty, MirSynthesizedFieldCopy::Primitive { field: id }) if ty.is_scalar_value() => {
                    *id == field.id
                }
                (
                    MirType::OptionalPrimitive(payload),
                    MirSynthesizedFieldCopy::OptionalPrimitive {
                        field: id,
                        payload: step_payload,
                    },
                ) => *id == field.id && payload == *step_payload,
                (
                    MirType::OptionalClass(target),
                    MirSynthesizedFieldCopy::OptionalClass {
                        field: id,
                        class: step_class,
                        operation,
                    },
                ) => {
                    *id == field.id
                        && target == *step_class
                        && self
                            .program
                            .class(target)
                            .and_then(|class| class.copy_constructor.selected())
                            == Some(*operation)
                }
                (MirType::Shared(_), MirSynthesizedFieldCopy::Shared { field: id }) => {
                    *id == field.id
                }
                (
                    MirType::OptionalShared(target),
                    MirSynthesizedFieldCopy::OptionalShared {
                        field: id,
                        target: step_target,
                    },
                ) => *id == field.id && target == *step_target,
                _ => false,
            };
            if !valid {
                self.program_error(format!(
                    "class {} synthesized copy-construction plan is invalid at field {}",
                    class.id, field.id
                ));
            }
        }
    }

    fn verify_synthesized_assignment(
        &mut self,
        class: &MirClassDeclaration,
        copy: &MirSynthesizedCopy<crate::identity::CopyAssignmentId>,
    ) {
        self.verify_assignment_base(class, copy.base);
        if copy.class != class.id || copy.fields.len() != class.fields.len() {
            self.program_error(format!(
                "class {} synthesized copy-assignment plan has the wrong owner or field count",
                class.id
            ));
            return;
        }
        for (field, step) in class.fields.iter().zip(&copy.fields) {
            let valid = match (field.ty, step) {
                (
                    MirType::Class(target),
                    MirSynthesizedFieldCopy::Class {
                        field: id,
                        operation,
                    },
                ) => {
                    *id == field.id
                        && self
                            .program
                            .class(target)
                            .and_then(|class| class.copy_assignment.selected())
                            == Some(*operation)
                }
                (ty, MirSynthesizedFieldCopy::Primitive { field: id }) if ty.is_scalar_value() => {
                    *id == field.id
                }
                (
                    MirType::OptionalPrimitive(payload),
                    MirSynthesizedFieldCopy::OptionalPrimitive {
                        field: id,
                        payload: step_payload,
                    },
                ) => *id == field.id && payload == *step_payload,
                (
                    MirType::OptionalClass(target),
                    MirSynthesizedFieldCopy::OptionalClass {
                        field: id,
                        class: step_class,
                        operation,
                    },
                ) => {
                    *id == field.id
                        && target == *step_class
                        && self
                            .program
                            .class(target)
                            .and_then(|class| class.copy_assignment.selected())
                            == Some(*operation)
                }
                (MirType::Shared(_), MirSynthesizedFieldCopy::Shared { field: id }) => {
                    *id == field.id
                }
                (
                    MirType::OptionalShared(target),
                    MirSynthesizedFieldCopy::OptionalShared {
                        field: id,
                        target: step_target,
                    },
                ) => *id == field.id && target == *step_target,
                _ => false,
            };
            if !valid {
                self.program_error(format!(
                    "class {} synthesized copy-assignment plan is invalid at field {}",
                    class.id, field.id
                ));
            }
        }
    }

    fn verify_parameters_declaration(&mut self, owner: &str, parameters: &[MirParameter]) {
        for (index, parameter) in parameters.iter().enumerate() {
            match parameter.mode {
                MirParameterMode::Value if parameter.ty == MirType::Unit => self.program_error(
                    format!("{owner} value parameter {index} cannot have type `unit`"),
                ),
                MirParameterMode::Value
                    if matches!(parameter.ty, MirType::Interface(_) | MirType::Obj) =>
                {
                    self.program_error(format!(
                        "{owner} value parameter {index} cannot have a non-owning interface or `Obj` type"
                    ));
                }
                MirParameterMode::ReadOnlyAlias | MirParameterMode::MutableAlias
                    if !matches!(
                        parameter.ty,
                        MirType::Class(_) | MirType::Interface(_) | MirType::Obj
                    ) =>
                {
                    self.program_error(format!(
                        "{owner} alias parameter {index} must have class, interface, or `Obj` type"
                    ));
                }
                _ => {}
            }
            if let MirType::Class(class) = parameter.ty {
                if self.program.class(class).is_none() {
                    self.program_error(format!(
                        "{owner} parameter {index} has undeclared class type {class}"
                    ));
                }
            }
            if let MirType::Interface(interface) = parameter.ty {
                if self.program.interface(interface).is_none() {
                    self.program_error(format!(
                        "{owner} parameter {index} has undeclared interface type {interface}"
                    ));
                }
            }
        }
    }

    fn verify_member_parameters(&mut self, owner: &str, parameters: &[MirParameter]) {
        self.verify_parameters_declaration(owner, parameters);
    }
}

#[cfg(test)]
mod tests;
