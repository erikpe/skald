//! Program-wide MIR declaration and definition-table verification.
//!
//! This module owns deterministic metadata traversal. It delegates executable
//! bodies through one focused verifier method and contains no block or
//! instruction verification.

use std::collections::HashSet;

use crate::{identity::CallableId, lexical_policy::is_source_identifier, source::Span};

use super::{
    super::model::{
        MirClassDeclaration, MirCopyCapability, MirDestructionStep, MirFunctionDeclaration,
        MirFunctionLinkage, MirParameter, MirParameterMode, MirReceiverAccess, MirSynthesizedCopy,
        MirSynthesizedFieldCopy, MirType,
    },
    context::Verifier,
};

impl<'mir> Verifier<'mir> {
    pub(super) fn verify_program(&mut self) {
        self.verify_module_ownership();
        self.verify_function_type_declarations();
        self.verify_closed_type_references();
        self.verify_external_links();
        self.verify_optional_declarations();
        self.verify_optional_box_declarations();
        self.verify_array_declarations();
        self.verify_classes();
        self.verify_string_declarations();
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
            if self.requires_complete_producer_definitions()
                && self
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
            if declaration.linkage != MirFunctionLinkage::Internal
                && (declaration
                    .parameters
                    .iter()
                    .any(|parameter| matches!(parameter.ty, MirType::Function(_)))
                    || matches!(declaration.return_type, MirType::Function(_)))
            {
                self.function_error(
                    declaration.id,
                    "non-internal function cannot transport function values",
                );
            }
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
            if let MirFunctionLinkage::External { link } = declaration.linkage {
                if declaration.parameters.iter().any(|parameter| {
                    parameter.mode != MirParameterMode::Value
                        || matches!(
                            parameter.ty,
                            MirType::Class(_)
                                | MirType::Array(_)
                                | MirType::Interface(_)
                                | MirType::Obj
                                | MirType::Shared(_)
                                | MirType::Optional(_)
                                | MirType::Function(_)
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
                        | MirType::Array(_)
                        | MirType::Interface(_)
                        | MirType::Obj
                        | MirType::Shared(_)
                        | MirType::Optional(_)
                        | MirType::Function(_)
                ) {
                    self.function_error(
                        declaration.id,
                        "external function cannot return an object value or shared owner",
                    );
                }
                let symbol = self
                    .program
                    .external_links
                    .get(link)
                    .map(|link| &link.symbol);
                if symbol.is_none_or(|symbol| {
                    symbol != &declaration.name || !is_source_identifier(symbol)
                }) {
                    self.function_error(
                        declaration.id,
                        "external link symbol must be the declaration's exact source identifier",
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
                    "non-internal function must not have a Skald definition",
                );
            }
            self.verify_definition(
                &declaration.parameters,
                declaration.return_type,
                definition.into(),
            );
        }

        if self.requires_complete_producer_definitions() {
            self.verify_producer_definition_completeness(&declarations);
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
                CallableId::Function(_) | CallableId::StaticInitializer(_) => None,
            };
            let Some((parameters, return_type)) = signature else {
                self.function_error(callable, "member definition has no matching declaration");
                continue;
            };
            self.verify_definition(parameters, return_type, definition.into());
        }

        if let Some(coordinator) = &self.program.static_lifecycle {
            for initializer in coordinator.initializers() {
                self.verify_definition(&[], MirType::Unit, initializer.into());
            }
        }
    }

    fn verify_producer_definition_completeness(
        &mut self,
        declarations: &[&MirFunctionDeclaration],
    ) {
        for declaration in declarations {
            if declaration.linkage == MirFunctionLinkage::Internal
                && self.program.definitions.get(declaration.id).is_none()
            {
                self.function_error(declaration.id, "internal function has no definition");
            }
        }

        let mut missing = Vec::new();
        for class in self.program.classes.iter() {
            for initializer in &class.initializers {
                missing.push((
                    initializer.id.into(),
                    format!("initializer {} has no member definition", initializer.id),
                ));
            }
            if let Some(copy) = &class.copy_constructor_declaration {
                missing.push((
                    copy.id.into(),
                    format!("copy constructor {} has no member definition", copy.id),
                ));
            }
            if let Some(copy) = &class.copy_assignment_declaration {
                missing.push((
                    copy.id.into(),
                    format!("copy assignment {} has no member definition", copy.id),
                ));
            }
            if let Some(destructor) = &class.destruction.destructor {
                missing.push((
                    destructor.id.into(),
                    format!("destructor {} has no member definition", destructor.id),
                ));
            }
            for method in &class.methods {
                missing.push((
                    method.id.into(),
                    format!("method {} has no member definition", method.id),
                ));
            }
        }
        for (callable, message) in missing {
            if self.program.member_definition(callable).is_none() {
                self.function_error(callable, message);
            }
        }
    }

    fn verify_external_links(&mut self) {
        let links = self.program.external_links.iter().collect::<Vec<_>>();
        if links
            .windows(2)
            .any(|pair| pair[0].symbol >= pair[1].symbol)
        {
            self.program_error("external-link symbols are not unique and ordered");
        }
        let mut symbols = HashSet::new();
        let mut linked_declarations = HashSet::new();
        for (index, link) in links.into_iter().enumerate() {
            if link.id.index() != index {
                self.program_error(format!(
                    "external-link table index {index} contains {}",
                    link.id
                ));
            }
            if !symbols.insert(link.symbol.as_str()) {
                self.program_error(format!("duplicate external symbol `{}`", link.symbol));
            }
            if !is_source_identifier(&link.symbol) {
                self.program_error(format!(
                    "external link {} has invalid source symbol `{}`",
                    link.id, link.symbol
                ));
            }
            if link.declarations.is_empty() {
                self.program_error(format!("external link {} has no declarations", link.id));
            }
            if link.declarations.windows(2).any(|pair| pair[0] >= pair[1]) {
                self.program_error(format!(
                    "external link {} declarations are not unique and ordered",
                    link.id
                ));
            }

            let mut signature = None;
            for &function in &link.declarations {
                if !linked_declarations.insert(function) {
                    self.function_error(function, "function occurs in multiple external links");
                }
                let Some(declaration) = self.program.declarations.get(function) else {
                    self.function_error(function, "external link references an unknown function");
                    continue;
                };
                if declaration.linkage != (MirFunctionLinkage::External { link: link.id }) {
                    self.function_error(
                        function,
                        format!("external link {} does not match function linkage", link.id),
                    );
                }
                if declaration.name != link.symbol {
                    self.function_error(
                        function,
                        format!(
                            "external link symbol `{}` differs from declaration name `{}`",
                            link.symbol, declaration.name
                        ),
                    );
                }
                let candidate = (&declaration.parameters, declaration.return_type);
                if let Some((parameters, result)) = signature {
                    if parameters != candidate.0 || result != candidate.1 {
                        self.function_error(
                            function,
                            format!(
                                "external link {} contains incompatible function signatures",
                                link.id
                            ),
                        );
                    }
                } else {
                    signature = Some(candidate);
                }
            }
        }

        for declaration in self.program.declarations.iter() {
            match declaration.linkage {
                MirFunctionLinkage::Internal if linked_declarations.contains(&declaration.id) => {
                    self.function_error(
                        declaration.id,
                        "internal function must not occur in an external link",
                    );
                }
                MirFunctionLinkage::External { link } => {
                    if self.program.external_links.get(link).is_none() {
                        self.function_error(
                            declaration.id,
                            format!("function references unknown external link {link}"),
                        );
                    } else if !linked_declarations.contains(&declaration.id) {
                        self.function_error(
                            declaration.id,
                            format!("function is absent from external link {link}"),
                        );
                    }
                }
                MirFunctionLinkage::Intrinsic { .. }
                    if linked_declarations.contains(&declaration.id) =>
                {
                    self.function_error(
                        declaration.id,
                        "intrinsic function must not occur in an external link",
                    );
                }
                MirFunctionLinkage::Intrinsic { .. } => {}
                MirFunctionLinkage::Internal => {}
            }
        }
    }

    fn verify_module_ownership(&mut self) {
        if let Err(error) = self.program.modules.validate() {
            self.program_error(error.to_string());
        }

        for declaration in self.program.declarations.iter() {
            if self.program.modules.get(declaration.module).is_none() {
                self.function_error(
                    declaration.id,
                    format!("function has unknown module owner {}", declaration.module),
                );
            }
        }
        for class in self.program.classes.iter() {
            if self.program.modules.get(class.module).is_none() {
                self.program_error(format!(
                    "class {} has unknown module owner {}",
                    class.id, class.module
                ));
            }
        }
        for interface in self.program.interfaces.iter() {
            if self.program.modules.get(interface.module).is_none() {
                self.program_error(format!(
                    "interface {} has unknown module owner {}",
                    interface.id, interface.module
                ));
            }
        }

        if let Some(entry) = self.program.declarations.get(self.program.entry_function) {
            if entry.module != self.program.modules.selected() {
                self.program_error(format!(
                    "entry function {} belongs to {}, but selected entry module is {}",
                    entry.id,
                    entry.module,
                    self.program.modules.selected()
                ));
            }
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
                if let Some(cell_span) = field.cell_span {
                    if cell_span.range().is_empty() || !span_contains(field.span, cell_span) {
                        self.program_error(format!(
                            "field {} cell modifier span must be nonempty and contained by its declaration span",
                            field.id
                        ));
                    }
                }
                if let Some(final_span) = field.final_span {
                    if final_span.range().is_empty() || !span_contains(field.span, final_span) {
                        self.program_error(format!(
                            "field {} final modifier span must be nonempty and contained by its declaration span",
                            field.id
                        ));
                    }
                }
                if field.cell_span.is_some() && field.final_span.is_some() {
                    self.program_error(format!(
                        "field {} cannot carry both cell and final metadata",
                        field.id
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
            for (index, field) in class.static_fields.iter().enumerate() {
                if field.id.class() != class.id || field.id.index() != index {
                    self.program_error(format!(
                        "class {} static-field table index {index} contains {}",
                        class.id, field.id
                    ));
                }
                if let Some(final_span) = field.final_span {
                    if final_span.range().is_empty() || !span_contains(field.span, final_span) {
                        self.program_error(format!(
                            "static field {} final modifier span must be nonempty and contained by its declaration span",
                            field.id
                        ));
                    }
                    if !matches!(
                        field.initialization,
                        super::super::model::MirStaticFieldInitialization::Explicit(_)
                    ) {
                        self.program_error(format!(
                            "final static field {} must have explicit initialization",
                            field.id
                        ));
                    }
                }
                if !self.static_field_type_is_supported(field.id, field.ty) {
                    self.program_error(format!(
                        "static field {} has unsupported MIR type {}",
                        field.id, field.ty
                    ));
                }
                if let MirType::Array(array) = field.ty {
                    if self.program.array_type(array).is_none() {
                        self.program_error(format!(
                            "static field {} has undeclared array type {array}",
                            field.id
                        ));
                    }
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
            }
            let mut expected_steps = Vec::new();
            if let Some(destructor) = &class.destruction.destructor {
                expected_steps.push(MirDestructionStep::UserBody(destructor.id));
            }
            expected_steps.extend(class.fields.iter().rev().filter_map(|field| {
                match field.ty {
                    MirType::Class(_) => Some(MirDestructionStep::Field(field.id)),
                    MirType::Shared(_) => Some(MirDestructionStep::SharedField(field.id)),
                    MirType::Optional(optional) => match self
                        .program
                        .optional_type(optional)
                        .map(|metadata| metadata.storage)
                    {
                        Some(crate::mir::MirOptionalStorage::SharedOwner(_)) => {
                            Some(MirDestructionStep::OptionalSharedField(field.id))
                        }
                        Some(crate::mir::MirOptionalStorage::InlineClass(_)) => {
                            Some(MirDestructionStep::OptionalClassField(field.id))
                        }
                        Some(
                            crate::mir::MirOptionalStorage::Nested(_)
                            | crate::mir::MirOptionalStorage::InlineArray(_),
                        ) => Some(MirDestructionStep::OptionalField {
                            field: field.id,
                            optional,
                        }),
                        _ => None,
                    },
                    MirType::Array(_) => Some(MirDestructionStep::ArrayField(field.id)),
                    _ => None,
                }
            }));
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
        if copy.class != class.id
            || copy.fields.len() != class.fields.len()
            || !copy.final_fields.is_empty()
        {
            self.program_error(format!(
                "class {} synthesized copy-construction plan has invalid owner, field count, or final-update evidence",
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
                    MirType::Optional(optional),
                    MirSynthesizedFieldCopy::OptionalPrimitive {
                        field: id,
                        payload: step_payload,
                    },
                ) => {
                    *id == field.id
                        && self
                            .program
                            .optional_type(optional)
                            .and_then(crate::mir::MirOptionalType::primitive)
                            == Some(*step_payload)
                }
                (
                    MirType::Optional(optional),
                    MirSynthesizedFieldCopy::OptionalClass {
                        field: id,
                        class: step_class,
                        operation,
                    },
                ) => {
                    *id == field.id
                        && self
                            .program
                            .optional_type(optional)
                            .and_then(crate::mir::MirOptionalType::inline_class)
                            == Some(*step_class)
                        && self
                            .program
                            .class(*step_class)
                            .and_then(|class| class.copy_constructor.selected())
                            == Some(*operation)
                }
                (MirType::Shared(_), MirSynthesizedFieldCopy::Shared { field: id }) => {
                    *id == field.id
                }
                (
                    MirType::Optional(optional),
                    MirSynthesizedFieldCopy::OptionalShared {
                        field: id,
                        target: step_target,
                    },
                ) => {
                    *id == field.id
                        && self
                            .program
                            .optional_type(optional)
                            .and_then(crate::mir::MirOptionalType::shared_owner)
                            == Some(*step_target)
                }
                (
                    MirType::Array(array),
                    MirSynthesizedFieldCopy::Array {
                        field: id,
                        array: step_array,
                    },
                ) => *id == field.id && array == *step_array,
                (
                    MirType::Optional(optional),
                    MirSynthesizedFieldCopy::Optional {
                        field: id,
                        optional: step_optional,
                    },
                ) => {
                    *id == field.id
                        && optional == *step_optional
                        && self
                            .program
                            .optional_type(optional)
                            .is_some_and(|metadata| {
                                matches!(
                                    metadata.storage,
                                    crate::mir::MirOptionalStorage::Nested(_)
                                        | crate::mir::MirOptionalStorage::InlineArray(_)
                                )
                            })
                }
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
        let expected_final_fields = class
            .fields
            .iter()
            .filter(|field| field.final_span.is_some())
            .map(|field| field.id)
            .collect::<Vec<_>>();
        if copy.final_fields != expected_final_fields {
            self.program_error(format!(
                "class {} synthesized copy-assignment plan has invalid final-update evidence",
                class.id
            ));
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
                    MirType::Optional(optional),
                    MirSynthesizedFieldCopy::OptionalPrimitive {
                        field: id,
                        payload: step_payload,
                    },
                ) => {
                    *id == field.id
                        && self
                            .program
                            .optional_type(optional)
                            .and_then(crate::mir::MirOptionalType::primitive)
                            == Some(*step_payload)
                }
                (
                    MirType::Optional(optional),
                    MirSynthesizedFieldCopy::OptionalClass {
                        field: id,
                        class: step_class,
                        operation,
                    },
                ) => {
                    *id == field.id
                        && self
                            .program
                            .optional_type(optional)
                            .and_then(crate::mir::MirOptionalType::inline_class)
                            == Some(*step_class)
                        && self
                            .program
                            .class(*step_class)
                            .and_then(|class| class.copy_assignment.selected())
                            == Some(*operation)
                }
                (MirType::Shared(_), MirSynthesizedFieldCopy::Shared { field: id }) => {
                    *id == field.id
                }
                (
                    MirType::Optional(optional),
                    MirSynthesizedFieldCopy::OptionalShared {
                        field: id,
                        target: step_target,
                    },
                ) => {
                    *id == field.id
                        && self
                            .program
                            .optional_type(optional)
                            .and_then(crate::mir::MirOptionalType::shared_owner)
                            == Some(*step_target)
                }
                (
                    MirType::Array(array),
                    MirSynthesizedFieldCopy::Array {
                        field: id,
                        array: step_array,
                    },
                ) => *id == field.id && array == *step_array,
                (
                    MirType::Optional(optional),
                    MirSynthesizedFieldCopy::Optional {
                        field: id,
                        optional: step_optional,
                    },
                ) => {
                    *id == field.id
                        && optional == *step_optional
                        && self
                            .program
                            .optional_type(optional)
                            .is_some_and(|metadata| {
                                matches!(
                                    metadata.storage,
                                    crate::mir::MirOptionalStorage::Nested(_)
                                        | crate::mir::MirOptionalStorage::InlineArray(_)
                                )
                            })
                }
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
                        MirType::I64
                            | MirType::U64
                            | MirType::U8
                            | MirType::F64
                            | MirType::Bool
                            | MirType::Class(_)
                            | MirType::Array(_)
                            | MirType::Shared(_)
                            | MirType::Interface(_)
                            | MirType::Obj
                            | MirType::Optional(_)
                    ) =>
                {
                    self.program_error(format!(
                        "{owner} alias parameter {index} must have primitive, owning, object-view, or optional type"
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
            if let MirType::Array(array) = parameter.ty {
                if self.program.array_type(array).is_none() {
                    self.program_error(format!(
                        "{owner} parameter {index} has undeclared array type {array}"
                    ));
                }
            }
        }
    }

    fn verify_member_parameters(&mut self, owner: &str, parameters: &[MirParameter]) {
        self.verify_parameters_declaration(owner, parameters);
    }
}

fn span_contains(outer: Span, inner: Span) -> bool {
    outer.source_id() == inner.source_id()
        && outer.range().start() <= inner.range().start()
        && inner.range().end() <= outer.range().end()
}

#[cfg(test)]
mod tests;
