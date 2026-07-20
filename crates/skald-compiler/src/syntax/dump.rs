//! Stable, source-independent textual rendering of the source AST.

use std::fmt::Write;

use crate::{
    dump_format::{write_indentation, write_quoted, write_span},
    source::Span,
};

use super::ast::*;

pub fn dump_ast(ast: &CompilationUnit) -> String {
    let mut dumper = AstDumper::default();
    dumper.line("CompilationUnit", ast.span);
    dumper.indented(|dumper| {
        for declaration in &ast.declarations {
            match declaration {
                TopLevelDeclaration::Function(function) => dumper.function(function),
                TopLevelDeclaration::ExternalFunction(function) => {
                    dumper.external_function(function)
                }
                TopLevelDeclaration::Class(class) => dumper.class(class),
            }
        }
    });
    dumper.output
}

#[derive(Default)]
struct AstDumper {
    output: String,
    indentation: usize,
}

impl AstDumper {
    fn class(&mut self, class: &ClassDecl) {
        self.line("Class", class.span);
        self.indented(|dumper| {
            dumper.named("Name", &class.name.text, class.name.span);
            dumper.heading("Members");
            dumper.indented(|dumper| {
                for member in &class.members {
                    dumper.class_member(member);
                }
            });
        });
    }

    fn class_member(&mut self, member: &ClassMember) {
        match member {
            ClassMember::Field(field) => {
                self.line("Field", field.span);
                self.indented(|dumper| {
                    dumper.named("Name", &field.name.text, field.name.span);
                    dumper.type_syntax(&field.type_syntax);
                });
            }
            ClassMember::Initializer(initializer) => {
                self.line("Initializer", initializer.span);
                self.indented(|dumper| {
                    dumper.line("Introducer", initializer.introducer_span);
                    dumper.parameters(&initializer.parameters);
                    dumper.block(&initializer.body);
                });
            }
            ClassMember::Method(method) => {
                self.line(
                    if method.mut_span.is_some() {
                        "Method Mutable"
                    } else {
                        "Method ReadOnly"
                    },
                    method.span,
                );
                self.indented(|dumper| {
                    if let Some(span) = method.mut_span {
                        dumper.line("Mut", span);
                    }
                    dumper.named("Name", &method.name.text, method.name.span);
                    dumper.parameters_and_return(&method.parameters, &method.return_type);
                    dumper.block(&method.body);
                });
            }
        }
    }

    fn function(&mut self, function: &FunctionDecl) {
        self.line("Function", function.span);
        self.indented(|dumper| {
            dumper.named("Name", &function.name.text, function.name.span);
            dumper.parameters_and_return(&function.parameters, &function.return_type);
            dumper.block(&function.body);
        });
    }

    fn external_function(&mut self, function: &ExternalFunctionDecl) {
        self.line("ExternalFunction", function.span);
        self.indented(|dumper| {
            dumper.named("Name", &function.name.text, function.name.span);
            dumper.parameters_and_return(&function.parameters, &function.return_type);
        });
    }

    fn parameters_and_return(&mut self, parameters: &[Parameter], return_type: &TypeSyntax) {
        self.parameters(parameters);
        self.heading("ReturnType");
        self.indented(|dumper| dumper.type_syntax(return_type));
    }

    fn parameters(&mut self, parameters: &[Parameter]) {
        self.heading("Parameters");
        self.indented(|dumper| {
            for parameter in parameters {
                dumper.line("Parameter", parameter.span);
                dumper.indented(|dumper| {
                    dumper.named("Name", &parameter.name.text, parameter.name.span);
                    dumper.type_syntax(&parameter.type_syntax);
                });
            }
        });
    }

    fn type_syntax(&mut self, type_syntax: &TypeSyntax) {
        let kind = match &type_syntax.kind {
            TypeKind::I64 => "I64",
            TypeKind::U64 => "U64",
            TypeKind::U8 => "U8",
            TypeKind::F64 => "F64",
            TypeKind::Bool => "Bool",
            TypeKind::Unit => "Unit",
            TypeKind::Named(name) => {
                self.named("Type Named", &name.text, type_syntax.span);
                return;
            }
        };
        self.line(&format!("Type {kind}"), type_syntax.span);
    }

    fn block(&mut self, block: &Block) {
        self.line("Block", block.span);
        self.indented(|dumper| {
            for statement in &block.statements {
                dumper.statement(statement);
            }
        });
    }

    fn statement(&mut self, statement: &Statement) {
        match statement {
            Statement::Local(local) => {
                self.line("Local", local.span);
                self.indented(|dumper| {
                    dumper.named("Name", &local.name.text, local.name.span);
                    dumper.type_syntax(&local.type_syntax);
                    dumper.heading("Initializer");
                    dumper.indented(|dumper| dumper.expression(&local.initializer));
                });
            }
            Statement::Return(statement) => {
                self.line("Return", statement.span);
                if let Some(value) = &statement.value {
                    self.indented(|dumper| dumper.expression(value));
                }
            }
            Statement::Expression(statement) => {
                self.line("ExpressionStatement", statement.span);
                self.indented(|dumper| dumper.expression(&statement.expression));
            }
            Statement::Conditional(statement) => self.conditional(statement),
            Statement::Block(block) => self.block(block),
            Statement::FieldAssignment(statement) => {
                self.line("FieldAssignment", statement.span);
                self.indented(|dumper| {
                    dumper.heading("Place");
                    dumper.indented(|dumper| dumper.member_access(&statement.place));
                    dumper.line("Equal", statement.equal_span);
                    dumper.heading("Value");
                    dumper.indented(|dumper| dumper.expression(&statement.value));
                });
            }
        }
    }

    fn conditional(&mut self, statement: &ConditionalStatement) {
        self.line("Conditional", statement.span);
        self.indented(|dumper| {
            dumper.conditional_arm("IfArm", &statement.if_arm);
            for arm in &statement.elif_arms {
                dumper.conditional_arm("ElifArm", arm);
            }
            if let Some(block) = &statement.else_block {
                dumper.heading("ElseArm");
                dumper.indented(|dumper| dumper.block(block));
            }
        });
    }

    fn conditional_arm(&mut self, name: &str, arm: &ConditionalArm) {
        self.line(name, arm.span);
        self.indented(|dumper| {
            dumper.heading("Condition");
            dumper.indented(|dumper| dumper.expression(&arm.condition));
            dumper.block(&arm.body);
        });
    }

    fn expression(&mut self, expression: &Expression) {
        match expression {
            Expression::Identifier(identifier) => {
                self.named("Identifier", &identifier.name.text, identifier.span);
            }
            Expression::NumericLiteral(literal) => {
                let name = match literal.kind {
                    crate::literal::NumericLiteralKind::I64 => "Integer",
                    crate::literal::NumericLiteralKind::U64 => "U64",
                    crate::literal::NumericLiteralKind::U8 => "U8",
                    crate::literal::NumericLiteralKind::F64 => "F64",
                };
                self.named(name, &literal.spelling, literal.span);
            }
            Expression::Boolean(boolean) => {
                self.line(
                    if boolean.value {
                        "Boolean true"
                    } else {
                        "Boolean false"
                    },
                    boolean.span,
                );
            }
            Expression::Unary(unary) => {
                let operator = match unary.operator {
                    UnaryOperator::Negate => "Negate",
                };
                self.line(&format!("Unary {operator}"), unary.span);
                self.indented(|dumper| dumper.expression(&unary.operand));
            }
            Expression::Binary(binary) => {
                let operator = match binary.operator {
                    BinaryOperator::Add => "Add",
                    BinaryOperator::Subtract => "Subtract",
                    BinaryOperator::Multiply => "Multiply",
                };
                self.line(&format!("Binary {operator}"), binary.span);
                self.indented(|dumper| {
                    dumper.expression(&binary.left);
                    dumper.expression(&binary.right);
                });
            }
            Expression::Call(call) => {
                self.line("Call", call.span);
                self.indented(|dumper| {
                    dumper.heading("Callee");
                    dumper.indented(|dumper| dumper.expression(&call.callee));
                    dumper.heading("Arguments");
                    dumper.indented(|dumper| {
                        for argument in &call.arguments {
                            dumper.expression(argument);
                        }
                    });
                });
            }
            Expression::Grouped(grouped) => {
                self.line("Grouped", grouped.span);
                self.indented(|dumper| dumper.expression(&grouped.expression));
            }
            Expression::SelfValue(self_value) => self.line("Self", self_value.span),
            Expression::MemberAccess(member) => self.member_access(member),
        }
    }

    fn member_access(&mut self, member: &MemberAccessExpr) {
        self.line("MemberAccess", member.span);
        self.indented(|dumper| {
            dumper.heading("Receiver");
            dumper.indented(|dumper| dumper.expression(&member.receiver));
            dumper.line("Dot", member.dot_span);
            dumper.named("Member", &member.member.text, member.member.span);
        });
    }

    fn heading(&mut self, name: &str) {
        self.write_indentation();
        let _ = writeln!(self.output, "{name}");
    }

    fn named(&mut self, kind: &str, text: &str, span: Span) {
        self.write_indentation();
        let _ = write!(self.output, "{kind} ");
        write_quoted(&mut self.output, text);
        write_span(&mut self.output, span);
        self.output.push('\n');
    }

    fn line(&mut self, kind: &str, span: Span) {
        self.write_indentation();
        self.output.push_str(kind);
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
