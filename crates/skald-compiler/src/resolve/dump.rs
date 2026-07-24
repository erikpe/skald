//! Deterministic textual rendering of the resolved program.

use std::fmt::Write;

use crate::{
    dump_format::{write_indentation, write_quoted, write_span},
    source::Span,
};

use super::ir::*;

pub fn dump_resolved(program: &ResolvedProgram) -> String {
    let mut dumper = ResolvedDumper::default();
    dumper.line("ResolvedProgram", program.span);
    dumper.indented(|dumper| {
        dumper.write_indentation();
        match program.entry_function {
            Some(function) => {
                let _ = writeln!(dumper.output, "Entry {function}");
            }
            None => dumper.output.push_str("Entry <none>\n"),
        }
        if !program.classes.is_empty() {
            dumper.heading("ClassDeclarations");
            dumper.indented(|dumper| {
                for class in program.classes.iter() {
                    dumper.class_declaration(class);
                }
            });
        }
        if !program.interfaces.is_empty() {
            dumper.heading("InterfaceDeclarations");
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
        if !program.classes.is_empty() {
            dumper.heading("ClassDefinitions");
            dumper.indented(|dumper| {
                for class in program.class_definitions.iter() {
                    dumper.class_definition(class);
                }
            });
        }
    });
    dumper.output
}

#[derive(Default)]
struct ResolvedDumper {
    output: String,
    indentation: usize,
}

impl ResolvedDumper {
    fn interface_declaration(&mut self, interface: &ResolvedInterfaceDeclaration) {
        self.write_indentation();
        let _ = write!(self.output, "Interface {} ", interface.id);
        write_quoted(&mut self.output, &interface.name);
        write_span(&mut self.output, interface.span);
        self.output.push('\n');
        self.indented(|dumper| {
            for requirement in &interface.requirements {
                dumper.write_indentation();
                let _ = write!(
                    dumper.output,
                    "Requirement {} {} ",
                    requirement.id,
                    if requirement.mutable {
                        "mutable"
                    } else {
                        "readonly"
                    },
                );
                write_quoted(&mut dumper.output, &requirement.name);
                write_span(&mut dumper.output, requirement.span);
                dumper.output.push('\n');
                dumper.indented(|dumper| {
                    for parameter in &requirement.parameters {
                        dumper.named_parameter(parameter);
                    }
                    dumper.heading("ReturnType");
                    dumper.indented(|dumper| dumper.type_syntax(&requirement.return_type));
                });
            }
        });
    }

    fn named_parameter(&mut self, parameter: &ResolvedInterfaceParameter) {
        self.write_indentation();
        self.output.push_str("Parameter ");
        write_quoted(&mut self.output, &parameter.name);
        write_span(&mut self.output, parameter.span);
        self.output.push('\n');
        self.indented(|dumper| dumper.type_syntax(&parameter.type_syntax));
    }

    fn class_declaration(&mut self, class: &ResolvedClassDeclaration) {
        self.write_indentation();
        let _ = write!(self.output, "Class {} ", class.id);
        write_quoted(&mut self.output, &class.name);
        write_span(&mut self.output, class.span);
        self.output.push('\n');
        self.indented(|dumper| {
            if let Some(base) = class.direct_base {
                dumper.line(&format!("DirectBase {}", base.class), base.span);
            }
            for claim in &class.implemented_interfaces {
                dumper.line(&format!("Implements {}", claim.interface), claim.span);
            }
            dumper.heading("Fields");
            dumper.indented(|dumper| {
                for field in &class.fields {
                    dumper.write_indentation();
                    let _ = write!(dumper.output, "Field {} ", field.id);
                    write_quoted(&mut dumper.output, &field.name);
                    write_span(&mut dumper.output, field.span);
                    dumper.output.push('\n');
                    dumper.indented(|dumper| dumper.type_syntax(&field.type_syntax));
                }
            });
            dumper.heading("OrdinaryInitializers");
            dumper.indented(|dumper| {
                for initializer in &class.initializers {
                    dumper.line(&format!("Initializer {}", initializer.id), initializer.span);
                    dumper.indented(|dumper| dumper.parameters(&initializer.parameters));
                }
            });
            dumper.heading("CopyConstructor");
            dumper.indented(|dumper| match class.copy_constructor {
                ResolvedCopyOperation::User(id) => {
                    let declaration = class
                        .copy_constructor_declaration
                        .as_ref()
                        .expect("user copy constructor must have declaration metadata");
                    dumper.line(&format!("User {id}"), declaration.span);
                    dumper.indented(|dumper| dumper.parameters(&declaration.parameters));
                }
                ResolvedCopyOperation::Synthesized(class) => {
                    dumper.raw_line(&format!("Synthesized {class}"));
                }
                ResolvedCopyOperation::Unavailable => dumper.raw_line("Unavailable"),
            });
            dumper.heading("CopyAssignment");
            dumper.indented(|dumper| match class.copy_assignment {
                ResolvedCopyOperation::User(id) => {
                    let declaration = class
                        .copy_assignment_declaration
                        .as_ref()
                        .expect("user copy assignment must have declaration metadata");
                    dumper.line(&format!("User {id}"), declaration.span);
                    dumper.indented(|dumper| {
                        dumper.parameters(std::slice::from_ref(&declaration.parameter))
                    });
                }
                ResolvedCopyOperation::Synthesized(class) => {
                    dumper.raw_line(&format!("Synthesized {class}"));
                }
                ResolvedCopyOperation::Unavailable => dumper.raw_line("Unavailable"),
            });
            dumper.heading("Destructor");
            if let Some(destructor) = &class.destructor {
                dumper.indented(|dumper| {
                    dumper.line(&format!("Destructor {}", destructor.id), destructor.span);
                });
            } else {
                dumper.indented(|dumper| dumper.raw_line("<none>"));
            }
            dumper.heading("Methods");
            dumper.indented(|dumper| {
                for method in &class.methods {
                    dumper.write_indentation();
                    let _ = write!(
                        dumper.output,
                        "Method {} {} ",
                        method.id,
                        match method.receiver_access {
                            ResolvedReceiverAccess::ReadOnly => "readonly",
                            ResolvedReceiverAccess::Mutable => "mutable",
                        }
                    );
                    write_quoted(&mut dumper.output, &method.name);
                    write_span(&mut dumper.output, method.span);
                    dumper.output.push('\n');
                    dumper.indented(|dumper| {
                        dumper.method_dispatch(method.dispatch);
                        dumper.parameters(&method.parameters);
                        dumper.heading("ReturnType");
                        dumper.indented(|dumper| dumper.type_syntax(&method.return_type));
                    });
                }
            });
        });
    }

    fn method_dispatch(&mut self, dispatch: ResolvedMethodDispatch) {
        match dispatch {
            ResolvedMethodDispatch::Direct => {}
            ResolvedMethodDispatch::VirtualRoot { family, slot } => {
                self.raw_line(&format!("Dispatch VirtualRoot {family} slot {slot}"));
            }
            ResolvedMethodDispatch::Override {
                family,
                slot,
                root,
                overridden,
            } => self.raw_line(&format!(
                "Dispatch Override {family} slot {slot} root {root} overridden {overridden}"
            )),
        }
    }

    fn class_definition(&mut self, class: &ResolvedClassDefinition) {
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

    fn member_definition(&mut self, definition: &ResolvedMemberDefinition) {
        self.line(
            &format!("MemberDefinition {}", definition.callable),
            definition.span,
        );
        self.indented(|dumper| {
            dumper.locals(&definition.locals);
            dumper.block(&definition.body);
        });
    }

    fn declaration(&mut self, declaration: &ResolvedFunctionDeclaration) {
        self.write_indentation();
        let _ = write!(self.output, "Declaration {} ", declaration.id);
        write_quoted(&mut self.output, &declaration.name);
        match &declaration.linkage {
            ResolvedFunctionLinkage::Internal => self.output.push_str(" internal"),
            ResolvedFunctionLinkage::External { symbol } => {
                self.output.push_str(" external ");
                write_quoted(&mut self.output, symbol);
            }
        }
        write_span(&mut self.output, declaration.span);
        self.output.push('\n');

        self.indented(|dumper| {
            dumper.parameters(&declaration.parameters);

            dumper.heading("ReturnType");
            dumper.indented(|dumper| dumper.type_syntax(&declaration.return_type));
        });
    }

    fn definition(&mut self, definition: &ResolvedFunctionDefinition) {
        self.line(
            &format!("Definition {}", definition.function),
            definition.span,
        );

        self.indented(|dumper| {
            dumper.locals(&definition.locals);
            dumper.block(&definition.body);
        });
    }

    fn parameters(&mut self, parameters: &[ResolvedParameter]) {
        self.heading("Parameters");
        self.indented(|dumper| {
            for parameter in parameters {
                dumper.write_indentation();
                let _ = write!(dumper.output, "Parameter {} ", parameter.id);
                write_quoted(&mut dumper.output, &parameter.name);
                write_span(&mut dumper.output, parameter.span);
                dumper.output.push('\n');
                dumper.indented(|dumper| {
                    dumper.parameter_binding_mode(parameter.binding_mode);
                    dumper.type_syntax(&parameter.type_syntax);
                });
            }
        });
    }

    fn parameter_binding_mode(&mut self, mode: ResolvedParameterBindingMode) {
        match mode {
            ResolvedParameterBindingMode::Value => self.heading("Binding Value"),
            ResolvedParameterBindingMode::ReadOnlyAlias { ref_span } => {
                self.heading("Binding ReadOnlyAlias");
                self.indented(|dumper| dumper.line("Ref", ref_span));
            }
            ResolvedParameterBindingMode::MutableAlias { mut_span, ref_span } => {
                self.heading("Binding MutableAlias");
                self.indented(|dumper| {
                    dumper.line("Mut", mut_span);
                    dumper.line("Ref", ref_span);
                });
            }
        }
    }

    fn locals(&mut self, locals: &[ResolvedLocal]) {
        self.heading("Locals");
        self.indented(|dumper| {
            for local in locals {
                dumper.write_indentation();
                let _ = write!(dumper.output, "Local {} ", local.id);
                write_quoted(&mut dumper.output, &local.name);
                write_span(&mut dumper.output, local.span);
                dumper.output.push('\n');
                dumper.indented(|dumper| dumper.type_syntax(&local.type_syntax));
            }
        });
    }

    fn type_syntax(&mut self, type_syntax: &ResolvedType) {
        let name = match type_syntax.kind {
            ResolvedTypeKind::I64 => "I64",
            ResolvedTypeKind::U64 => "U64",
            ResolvedTypeKind::U8 => "U8",
            ResolvedTypeKind::F64 => "F64",
            ResolvedTypeKind::Bool => "Bool",
            ResolvedTypeKind::Unit => "Unit",
            ResolvedTypeKind::Obj => "Obj",
            ResolvedTypeKind::Class(class) => {
                self.line(&format!("Type Class {class}"), type_syntax.span);
                return;
            }
            ResolvedTypeKind::Interface(interface) => {
                self.line(&format!("Type Interface {interface}"), type_syntax.span);
                return;
            }
        };
        self.line(&format!("Type {name}"), type_syntax.span);
    }

    fn block(&mut self, block: &ResolvedBlock) {
        self.line("Block", block.span);
        self.indented(|dumper| {
            for statement in &block.statements {
                dumper.statement(statement);
            }
        });
    }

    fn statement(&mut self, statement: &ResolvedStatement) {
        match statement {
            ResolvedStatement::BaseInitialization(statement) => {
                self.line(
                    &format!("BaseInitialization {}", statement.base),
                    statement.span,
                );
                self.indented(|dumper| {
                    dumper.line("Super", statement.super_span);
                    dumper.heading("Arguments");
                    dumper.indented(|dumper| {
                        for argument in &statement.arguments {
                            dumper.expression(argument);
                        }
                    });
                });
            }
            ResolvedStatement::Local(local) => {
                self.line(&format!("LocalDeclaration {}", local.local), local.span);
                self.indented(|dumper| dumper.expression(&local.initializer));
            }
            ResolvedStatement::Return(statement) => {
                self.line("Return", statement.span);
                if let Some(value) = &statement.value {
                    self.indented(|dumper| dumper.expression(value));
                }
            }
            ResolvedStatement::Expression(statement) => {
                self.line("ExpressionStatement", statement.span);
                self.indented(|dumper| dumper.expression(&statement.expression));
            }
            ResolvedStatement::Conditional(statement) => self.conditional(statement),
            ResolvedStatement::Block(block) => self.block(block),
            ResolvedStatement::FieldAssignment(assignment) => {
                self.line(
                    &format!("FieldAssignment {}", assignment.field),
                    assignment.span,
                );
                self.indented(|dumper| {
                    dumper.object_receiver(&assignment.receiver);
                    dumper.line("Equal", assignment.equal_span);
                    dumper.heading("Value");
                    dumper.indented(|dumper| dumper.expression(&assignment.value));
                });
            }
            ResolvedStatement::ObjectAssignment(assignment) => {
                self.line("ObjectAssignment", assignment.span);
                self.indented(|dumper| {
                    dumper.heading("Destination");
                    dumper.indented(|dumper| dumper.object_place(&assignment.destination));
                    dumper.heading("Source");
                    dumper.indented(|dumper| dumper.expression(&assignment.source));
                });
            }
        }
    }

    fn conditional(&mut self, statement: &ResolvedConditional) {
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

    fn expression(&mut self, expression: &ResolvedExpression) {
        match expression {
            ResolvedExpression::Binding(binding) => {
                self.line(&format!("Binding {}", binding.binding), binding.span);
            }
            ResolvedExpression::NumericLiteral(literal) => {
                self.write_indentation();
                self.output.push_str(match literal.kind {
                    crate::literal::NumericLiteralKind::I64 => "Integer ",
                    crate::literal::NumericLiteralKind::U64 => "U64 ",
                    crate::literal::NumericLiteralKind::U8 => "U8 ",
                    crate::literal::NumericLiteralKind::F64 => "F64 ",
                });
                write_quoted(&mut self.output, &literal.spelling);
                write_span(&mut self.output, literal.span);
                self.output.push('\n');
            }
            ResolvedExpression::Boolean(boolean) => {
                self.line(
                    if boolean.value {
                        "Boolean true"
                    } else {
                        "Boolean false"
                    },
                    boolean.span,
                );
            }
            ResolvedExpression::Unary(unary) => {
                let operator = match unary.operator {
                    ResolvedUnaryOperator::Negate => "Negate",
                };
                self.line(&format!("Unary {operator}"), unary.span);
                self.indented(|dumper| dumper.expression(&unary.operand));
            }
            ResolvedExpression::Binary(binary) => {
                let operator = match binary.operator {
                    ResolvedBinaryOperator::Add => "Add",
                    ResolvedBinaryOperator::Subtract => "Subtract",
                    ResolvedBinaryOperator::Multiply => "Multiply",
                };
                self.line(&format!("Binary {operator}"), binary.span);
                self.indented(|dumper| {
                    dumper.expression(&binary.left);
                    dumper.expression(&binary.right);
                });
            }
            ResolvedExpression::TypeTest(test) => {
                self.line(
                    &format!("TypeTest target {}", render_type_kind(test.target.kind)),
                    test.span,
                );
                self.indented(|dumper| dumper.expression(&test.source));
            }
            ResolvedExpression::ObjectCast(cast) => {
                let mode = match cast.target_mode {
                    ResolvedObjectCastTargetMode::Plain => "ObjectCast",
                    ResolvedObjectCastTargetMode::Shared { .. } => "SharedObjectCast",
                };
                self.line(
                    &format!("{mode} target {}", render_type_kind(cast.target.kind)),
                    cast.span,
                );
                self.indented(|dumper| dumper.expression(&cast.source));
            }
            ResolvedExpression::DirectCall(call) => {
                self.line(&format!("DirectCall {}", call.function), call.span);
                self.indented(|dumper| {
                    for argument in &call.arguments {
                        dumper.expression(argument);
                    }
                });
            }
            ResolvedExpression::Grouped(grouped) => {
                self.line("Grouped", grouped.span);
                self.indented(|dumper| dumper.expression(&grouped.expression));
            }
            ResolvedExpression::FieldAccess(access) => {
                self.line(&format!("FieldAccess {}", access.field), access.span);
                self.indented(|dumper| dumper.object_receiver(&access.receiver));
            }
            ResolvedExpression::MethodCall(call) => {
                self.line(&format!("MethodCall {}", call.method), call.span);
                self.indented(|dumper| {
                    dumper.object_receiver(&call.receiver);
                    dumper.heading("Arguments");
                    dumper.indented(|dumper| {
                        for argument in &call.arguments {
                            dumper.expression(argument);
                        }
                    });
                });
            }
            ResolvedExpression::InterfaceCall(call) => {
                let receiver = match &call.receiver {
                    ResolvedInterfaceReceiver::Binding { binding, .. } => {
                        format!("{binding}")
                    }
                    ResolvedInterfaceReceiver::Cast(_) => "checked-cast".to_owned(),
                };
                self.line(
                    &format!(
                        "InterfaceCall {} {} receiver {}",
                        call.interface, call.requirement, receiver
                    ),
                    call.span,
                );
                self.indented(|dumper| {
                    for argument in &call.arguments {
                        dumper.expression(argument);
                    }
                });
            }
            ResolvedExpression::Construct(construct) => match &construct.mode {
                ResolvedConstructionMode::Initialize { arguments } => {
                    self.line(&format!("Construct {}", construct.class), construct.span);
                    self.indented(|dumper| {
                        for argument in arguments {
                            dumper.expression(argument);
                        }
                    });
                }
                ResolvedConstructionMode::Copy { copy_span, source } => {
                    self.line(
                        &format!("CopyConstruct {}", construct.class),
                        construct.span,
                    );
                    self.indented(|dumper| {
                        dumper.line("Copy", *copy_span);
                        dumper.heading("Source");
                        dumper.indented(|dumper| dumper.expression(source));
                    });
                }
            },
        }
    }

    fn object_place(&mut self, place: &ResolvedObjectPlace) {
        self.line(
            &format!("Receiver {} class {}", place.render_identity(), place.class),
            place.span,
        );
    }

    fn object_receiver(&mut self, receiver: &ResolvedObjectReceiver) {
        match receiver {
            ResolvedObjectReceiver::BindingPath(path) => self.object_place(path),
            ResolvedObjectReceiver::CastRelative {
                cast,
                projections,
                class,
                span,
            } => {
                self.line(&format!("CastRelativeReceiver class {class}"), *span);
                self.indented(|dumper| {
                    dumper.line(
                        &format!("CastTarget {}", render_type_kind(cast.target.kind)),
                        cast.target_span,
                    );
                    dumper.heading("Source");
                    dumper.indented(|dumper| dumper.expression(&cast.source));
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
    }

    fn heading(&mut self, name: &str) {
        self.write_indentation();
        let _ = writeln!(self.output, "{name}");
    }

    fn raw_line(&mut self, text: &str) {
        self.write_indentation();
        let _ = writeln!(self.output, "{text}");
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

fn render_type_kind(kind: ResolvedTypeKind) -> String {
    match kind {
        ResolvedTypeKind::I64 => "i64".to_owned(),
        ResolvedTypeKind::U64 => "u64".to_owned(),
        ResolvedTypeKind::U8 => "u8".to_owned(),
        ResolvedTypeKind::F64 => "f64".to_owned(),
        ResolvedTypeKind::Bool => "bool".to_owned(),
        ResolvedTypeKind::Unit => "unit".to_owned(),
        ResolvedTypeKind::Obj => "Obj".to_owned(),
        ResolvedTypeKind::Class(class) => format!("class {class}"),
        ResolvedTypeKind::Interface(interface) => format!("interface {interface}"),
    }
}
