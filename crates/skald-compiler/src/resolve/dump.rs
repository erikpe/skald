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
struct ResolvedDumper {
    output: String,
    indentation: usize,
}

impl ResolvedDumper {
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
            dumper.heading("Parameters");
            dumper.indented(|dumper| {
                for parameter in &declaration.parameters {
                    dumper.write_indentation();
                    let _ = write!(dumper.output, "Parameter {} ", parameter.id);
                    write_quoted(&mut dumper.output, &parameter.name);
                    write_span(&mut dumper.output, parameter.span);
                    dumper.output.push('\n');
                    dumper.indented(|dumper| dumper.type_syntax(&parameter.type_syntax));
                }
            });

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
            dumper.heading("Locals");
            dumper.indented(|dumper| {
                for local in &definition.locals {
                    dumper.write_indentation();
                    let _ = write!(dumper.output, "Local {} ", local.id);
                    write_quoted(&mut dumper.output, &local.name);
                    write_span(&mut dumper.output, local.span);
                    dumper.output.push('\n');
                    dumper.indented(|dumper| dumper.type_syntax(&local.type_syntax));
                }
            });

            dumper.block(&definition.body);
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
        }
    }

    fn heading(&mut self, name: &str) {
        self.write_indentation();
        let _ = writeln!(self.output, "{name}");
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
