//! Deterministic textual rendering of typed HIR.

use std::fmt::{Display, Write};

use crate::{
    dump_format::{write_indentation, write_quoted, write_span},
    source::Span,
};

use super::ir::*;

pub fn dump_hir(program: &HirProgram) -> String {
    let mut dumper = HirDumper::default();
    dumper.line("HirProgram", program.span);
    dumper.indented(|dumper| {
        dumper.write_indentation();
        let _ = writeln!(dumper.output, "Entry {}", program.entry_function);
        if !program.classes.is_empty() {
            dumper.heading("Classes");
            dumper.indented(|dumper| {
                for class in program.classes.iter() {
                    dumper.class_declaration(class);
                }
            });
            dumper.heading("ClassDefinitions");
            dumper.indented(|dumper| {
                for class in program.class_definitions.iter() {
                    dumper.class_definition(class);
                }
            });
        }
        if !program.interfaces.is_empty() {
            dumper.heading("Interfaces");
            dumper.indented(|dumper| {
                for interface in program.interfaces.iter() {
                    dumper.interface_declaration(interface);
                }
            });
        }
        if !program.virtual_families.is_empty() {
            dumper.heading("VirtualFamilies");
            dumper.indented(|dumper| {
                for family in program.virtual_families.iter() {
                    dumper.raw_line(&format!(
                        "Family {} slot {} root {}",
                        family.id, family.slot, family.root
                    ));
                }
            });
        }
        dumper.heading("Declarations");
        dumper.indented(|dumper| {
            for declaration in program.declarations.iter() {
                dumper.declaration(declaration);
            }
        });
        dumper.heading("Definitions");
        dumper.indented(|dumper| {
            for definition in program.definitions.iter() {
                dumper.definition(definition);
            }
        });
    });
    dumper.output
}

#[derive(Default)]
struct HirDumper {
    output: String,
    indentation: usize,
}

impl HirDumper {
    fn interface_declaration(&mut self, interface: &HirInterfaceDeclaration) {
        self.write_indentation();
        let _ = write!(self.output, "Interface {} ", interface.id);
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
                        let _ = write!(dumper.output, " : {}", parameter.ty.name());
                        write_span(&mut dumper.output, parameter.span);
                        dumper.output.push('\n');
                    }
                });
            }
        });
    }

    fn class_declaration(&mut self, class: &HirClassDeclaration) {
        self.write_indentation();
        let _ = write!(self.output, "Class {} ", class.id);
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
                    write_quoted(&mut dumper.output, &field.name);
                    let _ = write!(dumper.output, " : {}", field.ty.name());
                    write_span(&mut dumper.output, field.span);
                    dumper.output.push('\n');
                }
            });
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
                            HirDestructionStep::OptionalClassField(field) => {
                                dumper.raw_line(&format!("OptionalClassField {field}"));
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
                    let access = match method.receiver_access {
                        HirAccess::ReadOnly => "readonly",
                        HirAccess::Mutable => "mutable",
                    };
                    dumper.write_indentation();
                    let _ = write!(dumper.output, "Method {} ", method.id);
                    write_quoted(&mut dumper.output, &method.name);
                    let _ = write!(dumper.output, " {access} -> {}", method.return_type.name());
                    write_span(&mut dumper.output, method.span);
                    dumper.output.push('\n');
                    dumper.indented(|dumper| {
                        dumper.method_dispatch(method.dispatch);
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

    fn class_definition(&mut self, class: &HirClassDefinition) {
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
                    for field in &operation.fields {
                        match field {
                            HirSynthesizedFieldCopy::Primitive { field } => {
                                dumper.raw_line(&format!("Primitive {field}"));
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
                        }
                    }
                });
            }
        }
    }

    fn declaration(&mut self, declaration: &HirFunctionDeclaration) {
        self.write_indentation();
        let _ = write!(self.output, "Declaration {} ", declaration.id);
        write_quoted(&mut self.output, &declaration.name);
        match &declaration.linkage {
            HirFunctionLinkage::Internal => self.output.push_str(" internal"),
            HirFunctionLinkage::External { symbol } => {
                self.output.push_str(" external ");
                write_quoted(&mut self.output, symbol);
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

    fn definition(&mut self, definition: &HirFunctionDefinition) {
        self.line(
            &format!("Definition {}", definition.function),
            definition.span,
        );

        self.indented(|dumper| {
            dumper.locals(&definition.locals);
            dumper.block(&definition.body);
        });
    }

    fn parameter(&mut self, parameter: &HirParameter) {
        self.write_indentation();
        let _ = write!(self.output, "Parameter {} ", parameter.id);
        write_quoted(&mut self.output, &parameter.name);
        let mode = match parameter.mode {
            HirParameterMode::Value => "value",
            HirParameterMode::ReadOnlyAlias => "ref",
            HirParameterMode::MutableAlias => "mut-ref",
        };
        let _ = write!(self.output, " {mode} : {}", parameter.ty.name());
        write_span(&mut self.output, parameter.span);
        self.output.push('\n');
    }

    fn locals(&mut self, locals: &[HirLocal]) {
        self.heading("Locals");
        self.indented(|dumper| {
            for local in locals {
                dumper.write_indentation();
                let _ = write!(dumper.output, "Local {} ", local.id);
                write_quoted(&mut dumper.output, &local.name);
                let _ = write!(dumper.output, " : {}", local.ty.name());
                write_span(&mut dumper.output, local.span);
                dumper.output.push('\n');
            }
        });
    }

    fn block(&mut self, block: &HirBlock) {
        self.line("Block", block.span);
        self.indented(|dumper| {
            for statement in &block.statements {
                dumper.statement(statement);
            }
        });
    }

    fn statement(&mut self, statement: &HirStatement) {
        match statement {
            HirStatement::BaseInitialization(statement) => {
                self.line(
                    &format!(
                        "BaseInitialization {} via {}",
                        statement.base, statement.initializer
                    ),
                    statement.span,
                );
                self.indented(|dumper| {
                    for argument in &statement.arguments {
                        dumper.call_argument(argument);
                    }
                });
            }
            HirStatement::Local(local) => {
                self.line(&format!("LocalDeclaration {}", local.local), local.span);
                self.indented(|dumper| match &local.initializer {
                    HirLocalInitializer::Value(expression) => dumper.expression(expression),
                    HirLocalInitializer::Object(initialization) => {
                        dumper.line("ObjectInitialization", initialization.span);
                        dumper.indented(|dumper| {
                            dumper.object_place(&initialization.destination);
                            dumper.object_producer(&initialization.producer);
                            if let Some(operation) = initialization.elided_copy {
                                dumper.raw_line("ElidedCopy");
                                dumper.indented(|dumper| dumper.selected_copy_operation(operation));
                            }
                        });
                    }
                    HirLocalInitializer::Copy(copy) => dumper.copy_construction(copy),
                    HirLocalInitializer::Shared(value) => dumper.shared_transfer(value),
                    HirLocalInitializer::Optional(source) => dumper.optional_source(source),
                    HirLocalInitializer::ClassOptional(value) => dumper.class_optional_value(value),
                });
            }
            HirStatement::Return(statement) => {
                self.line("Return", statement.span);
                if let Some(value) = &statement.value {
                    self.indented(|dumper| match value {
                        HirReturnValue::Scalar(value) => dumper.expression(value),
                        HirReturnValue::Object(HirObjectReturn::Copy {
                            source,
                            operation,
                            class,
                            span,
                        }) => {
                            dumper.line(&format!("ObjectResult {class}"), *span);
                            dumper.indented(|dumper| {
                                dumper.object_source(source);
                                dumper.selected_copy_operation(*operation);
                            });
                        }
                        HirReturnValue::Object(HirObjectReturn::Construct {
                            construction,
                            omitted_copy,
                        }) => {
                            let heading = if omitted_copy.is_some() {
                                "ElidedObjectResult"
                            } else {
                                "ObjectResult"
                            };
                            dumper.line(heading, construction.span);
                            dumper.indented(|dumper| {
                                dumper.construction(construction);
                                if let Some(operation) = omitted_copy {
                                    dumper.raw_line("ElidedCopy");
                                    dumper.indented(|dumper| {
                                        dumper.selected_copy_operation(*operation)
                                    });
                                }
                            });
                        }
                        HirReturnValue::Shared(value) => dumper.shared_transfer(value),
                        HirReturnValue::Optional(source) => dumper.optional_source(source),
                        HirReturnValue::ClassOptional(value) => dumper.class_optional_value(value),
                    });
                }
            }
            HirStatement::Call(statement) => {
                self.line("CallStatement", statement.span);
                self.indented(|dumper| dumper.expression(&statement.call));
            }
            HirStatement::Conditional(statement) => self.conditional(statement),
            HirStatement::Block(block) => self.block(block),
            HirStatement::FieldAssignment(statement) => {
                self.line("FieldAssignment", statement.span);
                self.indented(|dumper| {
                    dumper.field_place(&statement.place);
                    dumper.expression(&statement.value);
                });
            }
            HirStatement::FieldConstruction(statement) => {
                self.line("FieldConstruction", statement.span);
                self.indented(|dumper| {
                    dumper.field_place(&statement.place);
                    dumper.construction(&statement.construction);
                });
            }
            HirStatement::FieldCopyConstruction(statement) => {
                self.line("FieldCopyConstruction", statement.span);
                self.indented(|dumper| {
                    dumper.field_place(&statement.place);
                    dumper.object_source(&statement.source);
                    dumper.selected_copy_operation(statement.operation);
                });
            }
            HirStatement::FieldCopyAssignment(statement) => {
                self.line("FieldCopyAssignment", statement.span);
                self.indented(|dumper| {
                    dumper.field_place(&statement.place);
                    dumper.object_source(&statement.source);
                    dumper.selected_copy_operation(statement.operation);
                });
            }
            HirStatement::CopyAssignment(statement) => {
                self.line("CopyAssignmentStatement", statement.span);
                self.indented(|dumper| {
                    dumper.object_place(&statement.destination);
                    dumper.object_source(&statement.source);
                    dumper.selected_copy_operation(statement.operation);
                });
            }
            HirStatement::SharedFieldWrite(statement) => {
                self.line(
                    match statement.kind {
                        HirSharedFieldWriteKind::Initialize => "SharedFieldInitialization",
                        HirSharedFieldWriteKind::Assign => "SharedFieldAssignment",
                    },
                    statement.span,
                );
                self.indented(|dumper| {
                    dumper.field_place(&statement.place);
                    dumper.shared_transfer(&statement.value);
                });
            }
            HirStatement::SharedAssignment(assignment) => {
                self.line(
                    &format!("SharedAssignment {}", assignment.destination),
                    assignment.span,
                );
                self.indented(|dumper| dumper.shared_transfer(&assignment.value));
            }
            HirStatement::OptionalAssignment(assignment) => {
                self.line(
                    &format!(
                        "OptionalAssignment {:?} : {}?",
                        assignment.kind,
                        assignment.payload.name()
                    ),
                    assignment.span,
                );
                self.indented(|dumper| {
                    dumper.optional_place(&assignment.destination);
                    dumper.optional_source(&assignment.source);
                });
            }
            HirStatement::ClassOptionalAssignment(assignment) => {
                self.line(
                    &format!(
                        "ClassOptionalAssignment class {}?",
                        assignment.destination.class
                    ),
                    assignment.span,
                );
                self.indented(|dumper| {
                    dumper.class_optional_source(&assignment.source);
                });
            }
        }
    }

    fn copy_construction(&mut self, copy: &crate::hir::HirCopyConstruction) {
        self.line("CopyConstruction", copy.span);
        self.indented(|dumper| {
            dumper.object_place(&copy.destination);
            dumper.object_source(&copy.source);
            dumper.selected_copy_operation(copy.operation);
        });
    }

    fn selected_copy_operation<I: Display>(&mut self, operation: HirSelectedCopyOperation<I>) {
        match operation {
            HirSelectedCopyOperation::User(id) => self.raw_line(&format!("Operation User {id}")),
            HirSelectedCopyOperation::Synthesized(class) => {
                self.raw_line(&format!("Operation Synthesized {class}"));
            }
        }
    }

    fn conditional(&mut self, statement: &HirConditional) {
        self.line("Conditional", statement.span);
        self.indented(|dumper| {
            for (index, arm) in statement.arms.iter().enumerate() {
                dumper.line(if index == 0 { "IfArm" } else { "ElifArm" }, arm.span);
                dumper.indented(|dumper| {
                    dumper.heading("Condition");
                    dumper.indented(|dumper| dumper.expression(&arm.condition));
                    dumper.block(&arm.body);
                });
            }
            if let Some(block) = &statement.else_block {
                dumper.heading("ElseArm");
                dumper.indented(|dumper| dumper.block(block));
            }
        });
    }

    fn expression(&mut self, expression: &HirExpression) {
        match &expression.kind {
            HirExpressionKind::Binding(binding) => {
                self.typed_line(&format!("Binding {binding}"), expression);
            }
            HirExpressionKind::I64(value) => {
                self.typed_line(&format!("Integer {value}"), expression);
            }
            HirExpressionKind::U64(value) => {
                self.typed_line(&format!("U64 {value}"), expression);
            }
            HirExpressionKind::U8(value) => {
                self.typed_line(&format!("U8 {value}"), expression);
            }
            HirExpressionKind::F64Bits(bits) => {
                self.typed_line(&format!("F64 0x{bits:016x}"), expression);
            }
            HirExpressionKind::Boolean(value) => {
                self.typed_line(&format!("Boolean {value}"), expression);
            }
            HirExpressionKind::Unary { operation, operand } => {
                let operation = match operation {
                    HirUnaryOperation::NegateI64 => "NegateI64",
                    HirUnaryOperation::NegateF64 => "NegateF64",
                };
                self.typed_line(&format!("Unary {operation}"), expression);
                self.indented(|dumper| dumper.expression(operand));
            }
            HirExpressionKind::Binary {
                operation,
                left,
                right,
            } => {
                let operation = match operation {
                    HirBinaryOperation::AddI64 => "AddI64",
                    HirBinaryOperation::SubtractI64 => "SubtractI64",
                    HirBinaryOperation::MultiplyI64 => "MultiplyI64",
                    HirBinaryOperation::AddU64 => "AddU64",
                    HirBinaryOperation::SubtractU64 => "SubtractU64",
                    HirBinaryOperation::MultiplyU64 => "MultiplyU64",
                    HirBinaryOperation::AddU8 => "AddU8",
                    HirBinaryOperation::SubtractU8 => "SubtractU8",
                    HirBinaryOperation::MultiplyU8 => "MultiplyU8",
                    HirBinaryOperation::AddF64 => "AddF64",
                    HirBinaryOperation::SubtractF64 => "SubtractF64",
                    HirBinaryOperation::MultiplyF64 => "MultiplyF64",
                };
                self.typed_line(&format!("Binary {operation}"), expression);
                self.indented(|dumper| {
                    dumper.expression(left);
                    dumper.expression(right);
                });
            }
            HirExpressionKind::DirectCall {
                function,
                arguments,
            } => {
                self.typed_line(&format!("DirectCall {function}"), expression);
                self.indented(|dumper| {
                    for argument in arguments {
                        dumper.call_argument(argument);
                    }
                });
            }
            HirExpressionKind::Grouped(inner) => {
                self.typed_line("Grouped", expression);
                self.indented(|dumper| dumper.expression(inner));
            }
            HirExpressionKind::FieldRead(place) => {
                self.typed_line(&format!("FieldRead {}", place.field), expression);
                self.indented(|dumper| dumper.object_place(&place.receiver));
            }
            HirExpressionKind::MethodCall {
                receiver,
                target,
                arguments,
            } => {
                self.typed_line(&format!("MethodCall {}", method_target(target)), expression);
                self.indented(|dumper| {
                    dumper.method_receiver(receiver);
                    for argument in arguments {
                        dumper.call_argument(argument);
                    }
                });
            }
            HirExpressionKind::InterfaceCall {
                receiver,
                target,
                arguments,
            } => {
                self.typed_line(
                    &format!("InterfaceCall {} {}", target.interface, target.requirement),
                    expression,
                );
                self.indented(|dumper| {
                    dumper.interface_receiver(receiver);
                    for argument in arguments {
                        dumper.call_argument(argument);
                    }
                });
            }
            HirExpressionKind::TypeTest(test) => {
                let kind = match test.kind {
                    HirTypeTestKind::StaticSuccess => "static-success",
                    HirTypeTestKind::StaticFailure => "static-failure",
                    HirTypeTestKind::Runtime => "runtime",
                };
                self.typed_line(
                    &format!("TypeTest -> {} {kind}", view_target_name(test.target)),
                    expression,
                );
                self.indented(|dumper| dumper.object_view("ObjectView", &test.source));
            }
            HirExpressionKind::PresenceTest { source, kind } => {
                let kind = match kind {
                    crate::hir::HirPresenceTestKind::Some => "Some",
                    crate::hir::HirPresenceTestKind::None => "None",
                };
                self.typed_line(&format!("PresenceTest {kind}"), expression);
                self.indented(|dumper| dumper.optional_operand(source));
            }
            HirExpressionKind::Unwrap(source) => {
                self.typed_line("OptionalUnwrap", expression);
                self.indented(|dumper| dumper.optional_operand(source));
            }
        }
    }

    fn optional_source(&mut self, source: &crate::hir::HirOptionalSource) {
        match source {
            crate::hir::HirOptionalSource::Absent { span } => self.line("OptionalAbsent", *span),
            crate::hir::HirOptionalSource::Present(value) => {
                self.line("OptionalPresent", value.span);
                self.indented(|dumper| dumper.expression(value));
            }
            crate::hir::HirOptionalSource::Copy(place) => {
                self.line("OptionalCopy", place.span);
                self.indented(|dumper| dumper.optional_place(place));
            }
            crate::hir::HirOptionalSource::Produced(expression) => {
                self.line("OptionalProduced", expression.span);
                self.indented(|dumper| dumper.expression(expression));
            }
        }
    }

    fn class_optional_value(&mut self, value: &crate::hir::HirClassOptionalInitialize) {
        self.line(
            &format!("ClassOptionalInitialization class {}?", value.class),
            value.span,
        );
        self.indented(|dumper| dumper.class_optional_source(&value.source));
    }

    fn class_optional_source(&mut self, source: &crate::hir::HirClassOptionalSource) {
        match source {
            crate::hir::HirClassOptionalSource::Absent { span } => {
                self.line("ClassOptionalAbsent", *span)
            }
            crate::hir::HirClassOptionalSource::Present(source) => {
                self.line("ClassOptionalPresent", source.span());
                self.indented(|dumper| dumper.object_source(source));
            }
            crate::hir::HirClassOptionalSource::Copy(place) => {
                self.line(
                    &format!("ClassOptionalCopy class {}?", place.class),
                    place.span,
                );
            }
            crate::hir::HirClassOptionalSource::Produced(expression) => {
                self.line("ClassOptionalProduced", expression.span);
                self.indented(|dumper| dumper.expression(expression));
            }
        }
    }

    fn optional_operand(&mut self, operand: &crate::hir::HirOptionalOperand) {
        match operand {
            crate::hir::HirOptionalOperand::Place(place) => self.optional_place(place),
            crate::hir::HirOptionalOperand::Produced(expression) => {
                self.line("OptionalProduced", expression.span);
                self.indented(|dumper| dumper.expression(expression));
            }
            crate::hir::HirOptionalOperand::ClassPlace(place) => {
                self.line(
                    &format!("ClassOptionalPlace class {}?", place.class),
                    place.span,
                );
            }
            crate::hir::HirOptionalOperand::ClassProduced(expression) => {
                self.line("ClassOptionalProduced", expression.span);
                self.indented(|dumper| dumper.expression(expression));
            }
        }
    }

    fn optional_place(&mut self, place: &crate::hir::HirOptionalPlace) {
        match &place.storage {
            crate::hir::HirOptionalStorage::Binding(binding) => {
                self.line(&format!("OptionalPlace {binding}"), place.span);
            }
            crate::hir::HirOptionalStorage::Field(field) => {
                self.line("OptionalFieldPlace", place.span);
                self.indented(|dumper| dumper.field_place(field));
            }
        }
    }

    fn construction(&mut self, construction: &HirConstruction) {
        match &construction.mode {
            HirConstructionMode::Initialize {
                initializer,
                arguments,
            } => {
                self.line(
                    &format!("Construct {} via {initializer}", construction.class),
                    construction.span,
                );
                self.indented(|dumper| {
                    for argument in arguments {
                        dumper.call_argument(argument);
                    }
                });
            }
            HirConstructionMode::Copy { source, operation } => {
                self.line(
                    &format!("ExplicitCopyConstruct {}", construction.class),
                    construction.span,
                );
                self.indented(|dumper| {
                    dumper.object_source(source);
                    dumper.selected_copy_operation(*operation);
                });
            }
        }
    }

    fn object_call(&mut self, call: &HirObjectCall) {
        let target = match call.target {
            HirObjectCallTarget::Direct(function) => format!("function {function}"),
            HirObjectCallTarget::Method { target, .. } => {
                format!("method {}", method_target(&target))
            }
            HirObjectCallTarget::Interface { target, .. } => {
                format!("interface {} {}", target.interface, target.requirement)
            }
        };
        self.line(&format!("ObjectCall {target} -> {}", call.class), call.span);
        self.indented(|dumper| {
            if let HirObjectCallTarget::Method { receiver, .. } = &call.target {
                dumper.method_receiver(receiver);
            }
            if let HirObjectCallTarget::Interface { receiver, .. } = &call.target {
                dumper.interface_receiver(receiver);
            }
            for argument in &call.arguments {
                dumper.call_argument(argument);
            }
        });
    }

    fn interface_receiver(&mut self, receiver: &HirInterfaceReceiver) {
        match receiver {
            HirInterfaceReceiver::View(view) => {
                self.call_argument(&HirCallArgument::View(view.clone()))
            }
            HirInterfaceReceiver::Checked(view) => {
                self.call_argument(&HirCallArgument::CheckedView(view.clone()))
            }
        }
    }

    fn call_argument(&mut self, argument: &HirCallArgument) {
        match argument {
            HirCallArgument::Value(expression) => {
                self.line("ValueArgument", expression.span);
                self.indented(|dumper| dumper.expression(expression));
            }
            HirCallArgument::Optional { source, payload } => {
                self.line(
                    &format!("OptionalArgument {}?", payload.name()),
                    source.span(),
                );
                self.indented(|dumper| dumper.optional_source(source));
            }
            HirCallArgument::ClassOptional(value) => {
                self.line(
                    &format!("ClassOptionalArgument class {}?", value.class),
                    value.span,
                );
                self.indented(|dumper| dumper.class_optional_source(&value.source));
            }
            HirCallArgument::Place(place) => {
                self.line("PlaceArgument", place.span());
                self.indented(|dumper| dumper.object_place(place));
            }
            HirCallArgument::View(view) => {
                self.object_view("ViewArgument", view);
            }
            HirCallArgument::CheckedView(view) => {
                let kind = match view.kind {
                    HirCheckedObjectViewKind::Static => "static",
                    HirCheckedObjectViewKind::RuntimeTerminate => "runtime-terminate",
                };
                self.object_view(&format!("CheckedViewArgument {kind}"), &view.view);
            }
            HirCallArgument::Copy(copy) => {
                self.line("CopyArgument", copy.span);
                self.indented(|dumper| {
                    dumper.object_source(&copy.source);
                    dumper.selected_copy_operation(copy.operation);
                });
            }
            HirCallArgument::Shared(value) => {
                self.line("SharedArgument", value.span);
                self.indented(|dumper| dumper.shared_transfer(value));
            }
        }
    }

    fn shared_transfer(&mut self, value: &HirSharedTransfer) {
        let operation = match value.operation {
            HirOwnerTransfer::Copy => "Copy",
            HirOwnerTransfer::Adopt => "Adopt",
        };
        self.line(
            &format!(
                "SharedTransfer {operation} -> {}",
                shared_target_name(value.target)
            ),
            value.span,
        );
        self.indented(|dumper| dumper.shared_source(&value.source));
    }

    fn shared_source(&mut self, source: &HirSharedSource) {
        match source {
            HirSharedSource::Place(HirSharedPlace::Binding {
                binding,
                target,
                span,
            }) => self.line(
                &format!("SharedBinding {binding} : {}", shared_target_name(*target)),
                *span,
            ),
            HirSharedSource::Place(HirSharedPlace::Field {
                place,
                target,
                span,
            }) => {
                self.line(
                    &format!(
                        "SharedField {} : {}",
                        place.field,
                        shared_target_name(*target)
                    ),
                    *span,
                );
                self.indented(|dumper| dumper.object_place(&place.receiver));
            }
            HirSharedSource::Produced(HirSharedProducer::Allocation(allocation)) => {
                match &allocation.mode {
                    crate::hir::HirSharedAllocationMode::Initialize {
                        initializer,
                        arguments,
                    } => {
                        self.line(
                            &format!(
                                "SharedAllocation {} initialize via {}",
                                allocation.class, initializer
                            ),
                            allocation.span,
                        );
                        self.indented(|dumper| {
                            for argument in arguments {
                                dumper.call_argument(argument);
                            }
                        });
                    }
                    crate::hir::HirSharedAllocationMode::Copy { source, operation } => {
                        self.line(
                            &format!("SharedAllocation {} copy", allocation.class),
                            allocation.span,
                        );
                        self.indented(|dumper| {
                            dumper.selected_copy_operation(*operation);
                            dumper.object_source(source);
                        });
                    }
                }
            }
            HirSharedSource::Produced(HirSharedProducer::Call(call)) => {
                self.line("SharedCallResult", call.span);
                self.indented(|dumper| dumper.expression(call));
            }
            HirSharedSource::Produced(HirSharedProducer::Cast(cast)) => {
                let kind = match cast.kind {
                    crate::hir::HirSharedCastKind::Static => "static",
                    crate::hir::HirSharedCastKind::RuntimeTerminate => "runtime-terminate",
                };
                self.line(
                    &format!("SharedCast {kind} -> {}", shared_target_name(cast.target)),
                    cast.span,
                );
                self.indented(|dumper| dumper.shared_source(&cast.source));
            }
        }
    }

    fn object_view(&mut self, label: &str, view: &HirObjectView) {
        self.line(
            &format!(
                "{label} -> {} {}",
                view_target_name(view.target),
                access_name(view.access)
            ),
            view.span,
        );
        self.indented(|dumper| {
            match &view.source {
                HirViewSource::Place(place) => dumper.object_place(place),
                HirViewSource::Produced(producer) => {
                    dumper.line("ProducedView", producer.span());
                    dumper.indented(|dumper| dumper.object_producer(producer));
                }
                HirViewSource::Forwarded {
                    binding,
                    target,
                    access,
                    span,
                    ..
                } => dumper.line(
                    &format!(
                        "ForwardedView {binding} : {} {}",
                        view_target_name(*target),
                        access_name(*access)
                    ),
                    *span,
                ),
                HirViewSource::Shared {
                    binding,
                    target,
                    access,
                    span,
                    ..
                } => dumper.line(
                    &format!(
                        "SharedPointee {binding} : {} {}",
                        view_target_name(*target),
                        access_name(*access)
                    ),
                    *span,
                ),
                HirViewSource::AnchoredShared {
                    source,
                    target,
                    access,
                    span,
                    ..
                } => {
                    dumper.line(
                        &format!(
                            "AnchoredSharedPointee : {} {}",
                            view_target_name(*target),
                            access_name(*access)
                        ),
                        *span,
                    );
                    dumper.indented(|dumper| dumper.shared_source(source));
                }
                HirViewSource::OptionalPayload { view, projections } => {
                    dumper.line(
                        &format!(
                            "CheckedOptionalPayload class {} {}",
                            view.source.class(),
                            access_name(view.access)
                        ),
                        view.span,
                    );
                    dumper.indented(|dumper| {
                        dumper.optional_operand(&view.source);
                        for projection in projections {
                            match projection {
                                crate::object_path::ObjectProjection::Base(base) => {
                                    dumper.heading(&format!("BaseProjection {base}"));
                                }
                                crate::object_path::ObjectProjection::Field(field) => {
                                    dumper.heading(&format!("FieldProjection {field}"));
                                }
                            }
                        }
                    });
                }
            }
            dumper.object_origin(&view.origin);
        });
    }

    fn object_source(&mut self, source: &crate::hir::HirObjectSource) {
        match source {
            crate::hir::HirObjectSource::Place(place) => self.object_place(place),
            crate::hir::HirObjectSource::Produced(producer) => {
                self.line("MaterializedSource", producer.span());
                self.indented(|dumper| dumper.object_producer(producer));
            }
            crate::hir::HirObjectSource::Checked(view) => {
                let kind = match view.kind {
                    HirCheckedObjectViewKind::Static => "static",
                    HirCheckedObjectViewKind::RuntimeTerminate => "runtime-terminate",
                };
                self.line(
                    &format!(
                        "CheckedSource {kind} -> {} {}",
                        view_target_name(view.consumer_target),
                        access_name(view.consumer_access)
                    ),
                    view.span,
                );
                self.indented(|dumper| dumper.object_view("SelectedView", &view.view));
            }
            crate::hir::HirObjectSource::Slice(slice) => {
                let path = slice
                    .bases
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(" -> ");
                self.line(
                    &format!("SliceSource [{path}] -> {}", slice.target),
                    slice.span,
                );
                self.indented(|dumper| dumper.object_source(&slice.source));
            }
        }
    }

    fn object_producer(&mut self, producer: &crate::hir::HirObjectProducer) {
        match producer {
            crate::hir::HirObjectProducer::Construct(construction) => {
                self.construction(construction)
            }
            crate::hir::HirObjectProducer::Call(call) => self.object_call(call),
        }
    }

    fn field_place(&mut self, place: &HirFieldPlace) {
        self.line(&format!("FieldPlace {}", place.field), place.span);
        self.indented(|dumper| {
            if let Some(view) = &place.shared_view {
                dumper.object_view("SharedFieldReceiver", view);
            } else if let Some(view) = &place.optional_view {
                dumper.object_view("OptionalFieldReceiver", view);
            } else {
                dumper.object_place(&place.receiver);
            }
        });
    }

    fn object_place(&mut self, place: &HirObjectPlace) {
        let access = match place.access {
            HirAccess::ReadOnly => "readonly",
            HirAccess::Mutable => "mutable",
        };
        self.line(
            &format!(
                "ObjectPlace {} : class {} {access}",
                place.path.render_identity(),
                place.class()
            ),
            place.span(),
        );
    }

    fn method_receiver(&mut self, receiver: &HirMethodReceiver) {
        self.heading("Receiver");
        self.indented(|dumper| {
            if let Some(view) = &receiver.shared_view {
                dumper.object_view("SharedMethodReceiver", view);
            } else if let Some(view) = &receiver.optional_view {
                dumper.object_view("OptionalMethodReceiver", view);
            } else {
                dumper.object_place(&receiver.place);
                dumper.object_origin(&receiver.origin);
            }
        });
    }

    fn object_origin(&mut self, origin: &HirObjectOrigin) {
        match origin {
            HirObjectOrigin::Exact {
                complete,
                dynamic_class,
            } => {
                self.heading(&format!("Origin Exact dynamic {dynamic_class}"));
                self.indented(|dumper| dumper.object_place(complete));
            }
            HirObjectOrigin::Forwarded {
                binding,
                static_target,
                access,
                dispatch_limit,
                span,
            } => {
                let limit = dispatch_limit
                    .map(|class| format!(" limit {class}"))
                    .unwrap_or_default();
                self.line(
                    &format!(
                        "Origin Forwarded {binding} : {} {}{limit}",
                        view_target_name(*static_target),
                        access_name(*access)
                    ),
                    *span,
                );
            }
            HirObjectOrigin::Produced {
                dynamic_class,
                span,
            } => self.line(&format!("Origin Produced dynamic {dynamic_class}"), *span),
            HirObjectOrigin::Shared {
                binding,
                static_target,
                access,
                span,
            } => self.line(
                &format!(
                    "Origin Shared {binding} : {} {}",
                    view_target_name(*static_target),
                    access_name(*access)
                ),
                *span,
            ),
            HirObjectOrigin::AnchoredShared {
                static_target,
                access,
                span,
            } => self.line(
                &format!(
                    "Origin AnchoredShared : {} {}",
                    view_target_name(*static_target),
                    access_name(*access)
                ),
                *span,
            ),
        }
    }

    fn typed_line(&mut self, name: &str, expression: &HirExpression) {
        self.write_indentation();
        let _ = write!(self.output, "{name} : {}", expression.ty.name());
        write_span(&mut self.output, expression.span);
        self.output.push('\n');
    }

    fn heading(&mut self, name: &str) {
        self.write_indentation();
        let _ = writeln!(self.output, "{name}");
    }

    fn raw_line(&mut self, text: &str) {
        self.heading(text);
    }

    fn line(&mut self, name: &str, span: Span) {
        self.write_indentation();
        self.output.push_str(name);
        write_span(&mut self.output, span);
        self.output.push('\n');
    }

    fn write_indentation(&mut self) {
        write_indentation(&mut self.output, self.indentation);
    }

    fn indented(&mut self, write_contents: impl FnOnce(&mut Self)) {
        self.indentation += 1;
        write_contents(self);
        self.indentation -= 1;
    }
}

fn method_target(target: &HirMethodCallTarget) -> String {
    match target {
        HirMethodCallTarget::Direct(method) => format!("Direct {method}"),
        HirMethodCallTarget::Virtual {
            family,
            slot,
            selected,
        } => format!("Virtual {family} slot {slot} selected {selected}"),
    }
}

fn view_target_name(target: HirViewTarget) -> String {
    match target {
        HirViewTarget::Class(class) => format!("class {class}"),
        HirViewTarget::Interface(interface) => format!("interface {interface}"),
        HirViewTarget::Obj => "Obj".to_owned(),
    }
}

fn shared_target_name(target: HirSharedTarget) -> String {
    match target {
        HirSharedTarget::Class(class) => format!("shared class {class}"),
        HirSharedTarget::Interface(interface) => format!("shared interface {interface}"),
        HirSharedTarget::Obj => "shared Obj".to_owned(),
    }
}

const fn parameter_mode_name(mode: HirParameterMode) -> &'static str {
    match mode {
        HirParameterMode::Value => "value",
        HirParameterMode::ReadOnlyAlias => "ref",
        HirParameterMode::MutableAlias => "mut-ref",
    }
}

const fn access_name(access: HirAccess) -> &'static str {
    match access {
        HirAccess::ReadOnly => "readonly",
        HirAccess::Mutable => "mutable",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        identity::{BindingId, ClassId, FieldId, FunctionId, LocalId},
        object_path::ObjectPath,
        source::SourceDatabase,
    };

    #[test]
    fn object_place_dump_renders_the_complete_identity_path_exactly() {
        let mut sources = SourceDatabase::new();
        let source = sources.add("place.ska", "root.link.leaf");
        let span = sources.get(source).unwrap().span(0, 14).unwrap();
        let root = BindingId::Local(LocalId::new(FunctionId::new(0), 0));
        let path = ObjectPath::root(root, ClassId::new(2), span)
            .project_field(FieldId::new(ClassId::new(2), 0), ClassId::new(1), span)
            .project_field(FieldId::new(ClassId::new(1), 3), ClassId::new(0), span);
        let place = HirObjectPlace {
            path,
            access: HirAccess::ReadOnly,
        };
        let mut dumper = HirDumper::default();

        dumper.object_place(&place);

        assert_eq!(
            dumper.output,
            "ObjectPlace f0:l0 -> c2:field0 -> c1:field3 : class c0 readonly @0..14\n"
        );
    }
}
