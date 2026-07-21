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
    fn class_declaration(&mut self, class: &HirClassDeclaration) {
        self.write_indentation();
        let _ = write!(self.output, "Class {} ", class.id);
        write_quoted(&mut self.output, &class.name);
        write_span(&mut self.output, class.span);
        self.output.push('\n');
        self.indented(|dumper| {
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
            dumper.line(
                &format!("Initializer {}", class.initializer.id),
                class.initializer.span,
            );
            dumper.indented(|dumper| {
                for parameter in &class.initializer.parameters {
                    dumper.parameter(parameter);
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
                        for parameter in &method.parameters {
                            dumper.parameter(parameter);
                        }
                    });
                }
            });
        });
    }

    fn class_definition(&mut self, class: &HirClassDefinition) {
        self.line(&format!("ClassDefinition {}", class.class), class.span);
        self.indented(|dumper| {
            dumper.member_definition(&class.initializer);
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
            HirCopyCapability::User(id) => self.raw_line(&format!("User {id}")),
            HirCopyCapability::Unavailable => self.raw_line("Unavailable"),
            HirCopyCapability::Synthesized(operation) => {
                self.raw_line(&format!("Synthesized {}", operation.class));
                self.indented(|dumper| {
                    for field in &operation.fields {
                        match field {
                            HirSynthesizedFieldCopy::Primitive { field } => {
                                dumper.raw_line(&format!("Primitive {field}"));
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
            HirStatement::Local(local) => {
                self.line(&format!("LocalDeclaration {}", local.local), local.span);
                self.indented(|dumper| match &local.initializer {
                    HirLocalInitializer::Value(expression) => dumper.expression(expression),
                    HirLocalInitializer::Construct(construction) => {
                        dumper.construction(construction)
                    }
                });
            }
            HirStatement::Return(statement) => {
                self.line("Return", statement.span);
                if let Some(value) = &statement.value {
                    self.indented(|dumper| dumper.expression(value));
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
                    dumper.object_place(&statement.source);
                    dumper.selected_copy_operation(statement.operation);
                });
            }
            HirStatement::FieldCopyAssignment(statement) => {
                self.line("FieldCopyAssignment", statement.span);
                self.indented(|dumper| {
                    dumper.field_place(&statement.place);
                    dumper.object_place(&statement.source);
                    dumper.selected_copy_operation(statement.operation);
                });
            }
        }
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
                method,
                arguments,
            } => {
                self.typed_line(&format!("MethodCall {method}"), expression);
                self.indented(|dumper| {
                    dumper.object_place(receiver);
                    for argument in arguments {
                        dumper.call_argument(argument);
                    }
                });
            }
        }
    }

    fn construction(&mut self, construction: &HirConstruction) {
        self.line(
            &format!(
                "Construct {} via {}",
                construction.class, construction.initializer
            ),
            construction.span,
        );
        self.indented(|dumper| {
            for argument in &construction.arguments {
                dumper.call_argument(argument);
            }
        });
    }

    fn call_argument(&mut self, argument: &HirCallArgument) {
        match argument {
            HirCallArgument::Value(expression) => {
                self.line("ValueArgument", expression.span);
                self.indented(|dumper| dumper.expression(expression));
            }
            HirCallArgument::Place(place) => {
                self.line("PlaceArgument", place.span());
                self.indented(|dumper| dumper.object_place(place));
            }
        }
    }

    fn field_place(&mut self, place: &HirFieldPlace) {
        self.line(&format!("FieldPlace {}", place.field), place.span);
        self.indented(|dumper| dumper.object_place(&place.receiver));
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
            .project(FieldId::new(ClassId::new(2), 0), ClassId::new(1), span)
            .project(FieldId::new(ClassId::new(1), 3), ClassId::new(0), span);
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
