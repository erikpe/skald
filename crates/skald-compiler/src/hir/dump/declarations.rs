//! Rendering for HIR declarations, definitions, and lifecycle capabilities.

use std::fmt::{Display, Write};

use crate::dump_format::{write_quoted, write_span};

use super::super::ir::*;
use super::aggregate::{
    array_assignment_name, array_copy_name, array_default_name, array_destruction_name,
};
use super::ownership::optional_shared_target_name;
use super::HirDumper;

impl<'types> HirDumper<'types> {
    pub(super) fn array_type(&mut self, array: &HirArrayType) {
        self.raw_line(&format!(
            "ArrayType {} element {}",
            array.id,
            self.type_name(array.element)
        ));
        self.indented(|dumper| {
            dumper.raw_line(&format!(
                "Default {}",
                array
                    .lifecycle
                    .default
                    .map(array_default_name)
                    .unwrap_or_else(|| "unavailable".to_owned())
            ));
            dumper.raw_line(&format!(
                "Copy {}",
                array
                    .lifecycle
                    .copy
                    .map(array_copy_name)
                    .unwrap_or_else(|| "unavailable".to_owned())
            ));
            dumper.raw_line(&format!(
                "Assignment {}",
                array
                    .lifecycle
                    .assignment
                    .map(array_assignment_name)
                    .unwrap_or_else(|| "unavailable".to_owned())
            ));
            dumper.raw_line(&format!(
                "Destruction {}",
                array_destruction_name(array.lifecycle.destruction)
            ));
        });
    }

    pub(super) fn interface_declaration(&mut self, interface: &HirInterfaceDeclaration) {
        self.write_indentation();
        let _ = write!(
            self.output,
            "Interface {} module {} ",
            interface.id, interface.module
        );
        write_quoted(&mut self.output, &interface.name);
        write_span(&mut self.output, interface.span);
        self.output.push('\n');
        self.indented(|dumper| {
            for requirement in &interface.requirements {
                let access = match requirement.receiver_access {
                    HirAccess::ReadOnly => "readonly",
                    HirAccess::Mutable => "mutable",
                };
                dumper.write_indentation();
                let _ = write!(dumper.output, "Requirement {} {access} ", requirement.id);
                write_quoted(&mut dumper.output, &requirement.name);
                let _ = write!(dumper.output, " -> {}", requirement.return_type.name());
                write_span(&mut dumper.output, requirement.span);
                dumper.output.push('\n');
                dumper.indented(|dumper| {
                    for parameter in &requirement.parameters {
                        dumper.write_indentation();
                        let _ = write!(
                            dumper.output,
                            "Parameter {} ",
                            parameter_mode_name(parameter.mode)
                        );
                        write_quoted(&mut dumper.output, &parameter.name);
                        let _ = write!(dumper.output, " : {}", dumper.type_name(parameter.ty));
                        write_span(&mut dumper.output, parameter.span);
                        dumper.output.push('\n');
                    }
                });
            }
        });
    }

    pub(super) fn class_declaration(&mut self, class: &HirClassDeclaration) {
        self.write_indentation();
        let _ = write!(self.output, "Class {} module {} ", class.id, class.module);
        write_quoted(&mut self.output, &class.name);
        write_span(&mut self.output, class.span);
        self.output.push('\n');
        self.indented(|dumper| {
            if let Some(base) = &class.direct_base {
                dumper.line(&format!("DirectBase {}", base.class), base.span);
            }
            if !class.conformances.is_empty() {
                dumper.heading("Conformances");
                dumper.indented(|dumper| {
                    for conformance in &class.conformances {
                        dumper.raw_line(&format!("Interface {}", conformance.interface));
                        dumper.indented(|dumper| {
                            for implementation in &conformance.implementations {
                                dumper.raw_line(&format!(
                                    "{} -> {}",
                                    implementation.requirement, implementation.method
                                ));
                            }
                        });
                    }
                });
            }
            dumper.heading("Fields");
            dumper.indented(|dumper| {
                for field in &class.fields {
                    dumper.write_indentation();
                    let _ = write!(dumper.output, "Field {} ", field.id);
                    if field.cell_span.is_some() {
                        dumper.output.push_str("cell ");
                    }
                    if field.final_span.is_some() {
                        dumper.output.push_str("final ");
                    }
                    write_quoted(&mut dumper.output, &field.name);
                    let _ = write!(dumper.output, " : {}", dumper.type_name(field.ty));
                    write_span(&mut dumper.output, field.span);
                    dumper.output.push('\n');
                    if let Some(span) = field.cell_span {
                        dumper.indented(|dumper| dumper.line("Cell", span));
                    }
                    if let Some(span) = field.final_span {
                        dumper.indented(|dumper| dumper.line("Final", span));
                    }
                }
            });
            if !class.static_fields.is_empty() {
                dumper.heading("StaticFields");
                dumper.indented(|dumper| {
                    for field in &class.static_fields {
                        dumper.write_indentation();
                        let _ = write!(dumper.output, "StaticField {} ", field.id);
                        if field.final_span.is_some() {
                            dumper.output.push_str("final ");
                        }
                        write_quoted(&mut dumper.output, &field.name);
                        let _ = write!(dumper.output, " : {}", dumper.type_name(field.ty));
                        write_span(&mut dumper.output, field.span);
                        dumper.output.push('\n');
                        dumper.indented(|dumper| {
                            if let Some(span) = field.final_span {
                                dumper.line("Final", span);
                            }
                            match &field.initializer {
                                Some(initializer) => {
                                    dumper.line(
                                        &format!(
                                            "DeclarationInitializer {} destination {}",
                                            initializer.id,
                                            dumper.type_name(field.ty)
                                        ),
                                        initializer.span,
                                    );
                                    dumper.indented(|dumper| {
                                        dumper.line("Equal", initializer.equal_span);
                                        dumper.stored_value_initialization(&initializer.value);
                                    });
                                }
                                None => dumper.raw_line("ZeroDefaultInitialization"),
                            }
                        });
                    }
                });
            }
            dumper.heading("Initializers");
            dumper.indented(|dumper| {
                for initializer in &class.initializers {
                    dumper.line(&format!("Initializer {}", initializer.id), initializer.span);
                    dumper.indented(|dumper| {
                        for parameter in &initializer.parameters {
                            dumper.parameter(parameter);
                        }
                    });
                }
            });
            dumper.heading("CopyConstructor");
            dumper.indented(|dumper| {
                dumper.copy_capability(&class.copy_constructor);
                if let Some(declaration) = &class.copy_constructor_declaration {
                    for parameter in &declaration.parameters {
                        dumper.parameter(parameter);
                    }
                }
            });
            dumper.heading("CopyAssignment");
            dumper.indented(|dumper| {
                dumper.copy_capability(&class.copy_assignment);
                if let Some(declaration) = &class.copy_assignment_declaration {
                    dumper.parameter(&declaration.parameter);
                }
            });
            if let Some(destructor) = &class.destructor {
                let access = match destructor.receiver_access {
                    HirAccess::ReadOnly => "readonly",
                    HirAccess::Mutable => "mutable",
                };
                dumper.line(
                    &format!("Destructor {} {access} -> unit", destructor.id),
                    destructor.span,
                );
            }
            if !class.destruction.steps.is_empty() {
                dumper.heading("DestructionPlan");
                dumper.indented(|dumper| {
                    for step in &class.destruction.steps {
                        match step {
                            HirDestructionStep::UserBody(destructor) => {
                                dumper.raw_line(&format!("UserBody {destructor}"));
                            }
                            HirDestructionStep::Field(field) => {
                                dumper.raw_line(&format!("Field {field}"));
                            }
                            HirDestructionStep::SharedField(field) => {
                                dumper.raw_line(&format!("SharedField {field}"));
                            }
                            HirDestructionStep::OptionalSharedField(field) => {
                                dumper.raw_line(&format!("OptionalSharedField {field}"));
                            }
                            HirDestructionStep::OptionalClassField(field) => {
                                dumper.raw_line(&format!("OptionalClassField {field}"));
                            }
                            HirDestructionStep::OptionalField { field, optional } => {
                                dumper.raw_line(&format!("OptionalField {field} {optional}"));
                            }
                            HirDestructionStep::ArrayField(field) => {
                                dumper.raw_line(&format!("ArrayField {field}"));
                            }
                            HirDestructionStep::Base(base) => {
                                dumper.raw_line(&format!("Base {base}"));
                            }
                        }
                    }
                });
            }
            dumper.heading("Methods");
            dumper.indented(|dumper| {
                for method in &class.methods {
                    let kind = match method.kind {
                        HirMethodKind::Instance {
                            receiver_access: HirAccess::ReadOnly,
                            ..
                        } => "readonly",
                        HirMethodKind::Instance {
                            receiver_access: HirAccess::Mutable,
                            ..
                        } => "mutable",
                        HirMethodKind::Static => "static",
                    };
                    dumper.write_indentation();
                    let _ = write!(dumper.output, "Method {} ", method.id);
                    write_quoted(&mut dumper.output, &method.name);
                    let _ = write!(dumper.output, " {kind} -> {}", method.return_type.name());
                    write_span(&mut dumper.output, method.span);
                    dumper.output.push('\n');
                    dumper.indented(|dumper| {
                        if let Some(dispatch) = method.kind.dispatch() {
                            dumper.method_dispatch(dispatch);
                        }
                        for parameter in &method.parameters {
                            dumper.parameter(parameter);
                        }
                    });
                }
            });
        });
    }

    fn method_dispatch(&mut self, dispatch: HirMethodDispatch) {
        match dispatch {
            HirMethodDispatch::Direct => {}
            HirMethodDispatch::VirtualRoot { family, slot } => {
                self.raw_line(&format!("Dispatch VirtualRoot {family} slot {slot}"));
            }
            HirMethodDispatch::Override {
                family,
                slot,
                root,
                overridden,
            } => self.raw_line(&format!(
                "Dispatch Override {family} slot {slot} root {root} overridden {overridden}"
            )),
        }
    }

    pub(super) fn class_definition(&mut self, class: &HirClassDefinition) {
        self.line(&format!("ClassDefinition {}", class.class), class.span);
        self.indented(|dumper| {
            for initializer in &class.initializers {
                dumper.member_definition(initializer);
            }
            if let Some(copy_constructor) = &class.copy_constructor {
                dumper.member_definition(copy_constructor);
            }
            if let Some(copy_assignment) = &class.copy_assignment {
                dumper.member_definition(copy_assignment);
            }
            if let Some(destructor) = &class.destructor {
                dumper.member_definition(destructor);
            }
            for method in &class.methods {
                dumper.member_definition(method);
            }
        });
    }

    fn member_definition(&mut self, definition: &HirMemberDefinition) {
        self.line(
            &format!("MemberDefinition {}", definition.callable),
            definition.span,
        );
        self.indented(|dumper| {
            dumper.locals(&definition.locals);
            dumper.block(&definition.body);
        });
    }

    fn copy_capability<I: Copy + Display>(&mut self, capability: &HirCopyCapability<I>) {
        match capability {
            HirCopyCapability::User(copy) => {
                self.raw_line(&format!("User {}", copy.operation));
                if let Some(base) = copy.base {
                    self.indented(|dumper| {
                        dumper.raw_line(&format!("Base {}", base.base));
                        dumper.indented(|dumper| dumper.selected_copy_operation(base.operation));
                    });
                }
            }
            HirCopyCapability::Unavailable => self.raw_line("Unavailable"),
            HirCopyCapability::Synthesized(operation) => {
                self.raw_line(&format!("Synthesized {}", operation.class));
                self.indented(|dumper| {
                    if let Some(base) = operation.base {
                        dumper.raw_line(&format!("Base {}", base.base));
                        dumper.indented(|dumper| dumper.selected_copy_operation(base.operation));
                    }
                    for field in &operation.final_fields {
                        dumper.raw_line(&format!("FinalField {field}"));
                    }
                    for field in &operation.fields {
                        match field {
                            HirSynthesizedFieldCopy::Scalar { field } => {
                                dumper.raw_line(&format!("Scalar {field}"));
                            }
                            HirSynthesizedFieldCopy::OptionalPrimitive { field, payload } => {
                                dumper.raw_line(&format!(
                                    "OptionalPrimitive {field} : {}?",
                                    payload.name()
                                ));
                            }
                            HirSynthesizedFieldCopy::Shared { field } => {
                                dumper.raw_line(&format!("Shared {field}"));
                            }
                            HirSynthesizedFieldCopy::OptionalShared { field, target } => {
                                dumper.raw_line(&format!(
                                    "OptionalShared {field} : {}",
                                    optional_shared_target_name(*target)
                                ));
                            }
                            HirSynthesizedFieldCopy::Optional { field, optional } => {
                                dumper.raw_line(&format!("Optional {field} : {optional}"));
                            }
                            HirSynthesizedFieldCopy::OptionalClass {
                                field,
                                class,
                                operation,
                            } => {
                                dumper.raw_line(&format!("OptionalClass {field} : class {class}?"));
                                dumper
                                    .indented(|dumper| dumper.selected_copy_operation(*operation));
                            }
                            HirSynthesizedFieldCopy::Class { field, operation } => {
                                let selected = match operation {
                                    HirSelectedCopyOperation::User(id) => format!("User {id}"),
                                    HirSelectedCopyOperation::Synthesized(class) => {
                                        format!("Synthesized {class}")
                                    }
                                };
                                dumper.raw_line(&format!("Class {field} using {selected}"));
                            }
                            HirSynthesizedFieldCopy::Array { field, array } => {
                                dumper.raw_line(&format!("Array {field} : {array}"));
                            }
                        }
                    }
                });
            }
        }
    }

    pub(super) fn declaration(&mut self, declaration: &HirFunctionDeclaration) {
        self.write_indentation();
        let _ = write!(
            self.output,
            "Declaration {} module {} ",
            declaration.id, declaration.module
        );
        write_quoted(&mut self.output, &declaration.name);
        match &declaration.linkage {
            HirFunctionLinkage::Internal => self.output.push_str(" internal"),
            HirFunctionLinkage::External { link } => {
                let _ = write!(self.output, " external {link}");
            }
            HirFunctionLinkage::Intrinsic { intrinsic } => {
                let _ = write!(self.output, " intrinsic {intrinsic:?}");
            }
        }
        write_span(&mut self.output, declaration.span);
        self.output.push('\n');

        self.indented(|dumper| {
            dumper.heading("Parameters");
            dumper.indented(|dumper| {
                for parameter in &declaration.parameters {
                    dumper.parameter(parameter);
                }
            });

            dumper.write_indentation();
            let _ = writeln!(
                dumper.output,
                "ReturnType {}",
                declaration.return_type.name()
            );
        });
    }

    pub(super) fn definition(&mut self, definition: &HirFunctionDefinition) {
        self.line(
            &format!("Definition {}", definition.function),
            definition.span,
        );

        self.indented(|dumper| {
            dumper.locals(&definition.locals);
            dumper.block(&definition.body);
        });
    }

    pub(super) fn parameter(&mut self, parameter: &HirParameter) {
        self.write_indentation();
        let _ = write!(self.output, "Parameter {} ", parameter.id);
        write_quoted(&mut self.output, &parameter.name);
        let mode = match parameter.mode {
            HirParameterMode::Value => "value",
            HirParameterMode::ReadOnlyAlias => "ref",
            HirParameterMode::MutableAlias => "mut-ref",
        };
        let _ = write!(self.output, " {mode} : {}", self.type_name(parameter.ty));
        write_span(&mut self.output, parameter.span);
        self.output.push('\n');
    }
}

const fn parameter_mode_name(mode: HirParameterMode) -> &'static str {
    match mode {
        HirParameterMode::Value => "value",
        HirParameterMode::ReadOnlyAlias => "ref",
        HirParameterMode::MutableAlias => "mut-ref",
    }
}
