//! Deterministic textual rendering of typed HIR.

use std::fmt::Write;

use crate::source::Span;

use super::ir::*;

pub fn dump_hir(program: &HirProgram) -> String {
    let mut dumper = HirDumper::default();
    dumper.line("HirProgram", program.span);
    dumper.indented(|dumper| {
        dumper.write_indentation();
        let _ = writeln!(dumper.output, "Entry {}", program.entry_function);
        dumper.heading("Functions");
        dumper.indented(|dumper| {
            for function in program.functions.iter() {
                dumper.function(function);
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
    fn function(&mut self, function: &HirFunction) {
        self.write_indentation();
        let _ = write!(self.output, "Function {} ", function.id);
        write_quoted(&mut self.output, &function.name);
        write_span(&mut self.output, function.span);
        self.output.push('\n');

        self.indented(|dumper| {
            dumper.heading("Parameters");
            dumper.indented(|dumper| {
                for parameter in &function.parameters {
                    dumper.write_indentation();
                    let _ = write!(dumper.output, "Parameter {} ", parameter.id);
                    write_quoted(&mut dumper.output, &parameter.name);
                    let _ = write!(dumper.output, " : {}", parameter.ty.name());
                    write_span(&mut dumper.output, parameter.span);
                    dumper.output.push('\n');
                }
            });

            dumper.write_indentation();
            let _ = writeln!(dumper.output, "ReturnType {}", function.return_type.name());

            dumper.heading("Locals");
            dumper.indented(|dumper| {
                for local in &function.locals {
                    dumper.write_indentation();
                    let _ = write!(dumper.output, "Local {} ", local.id);
                    write_quoted(&mut dumper.output, &local.name);
                    let _ = write!(dumper.output, " : {}", local.ty.name());
                    write_span(&mut dumper.output, local.span);
                    dumper.output.push('\n');
                }
            });

            dumper.block(&function.body);
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
                self.indented(|dumper| dumper.expression(&local.initializer));
            }
            HirStatement::Return(statement) => {
                self.line("Return", statement.span);
                self.indented(|dumper| dumper.expression(&statement.value));
            }
            HirStatement::Block(block) => self.block(block),
        }
    }

    fn expression(&mut self, expression: &HirExpression) {
        match &expression.kind {
            HirExpressionKind::Binding(binding) => {
                self.typed_line(&format!("Binding {binding}"), expression);
            }
            HirExpressionKind::Integer(value) => {
                self.typed_line(&format!("Integer {value}"), expression);
            }
            HirExpressionKind::Unary { operation, operand } => {
                let operation = match operation {
                    HirUnaryOperation::NegateI64 => "NegateI64",
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
                        dumper.expression(argument);
                    }
                });
            }
            HirExpressionKind::Grouped(inner) => {
                self.typed_line("Grouped", expression);
                self.indented(|dumper| dumper.expression(inner));
            }
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

    fn line(&mut self, name: &str, span: Span) {
        self.write_indentation();
        self.output.push_str(name);
        write_span(&mut self.output, span);
        self.output.push('\n');
    }

    fn write_indentation(&mut self) {
        for _ in 0..self.indentation {
            self.output.push_str("  ");
        }
    }

    fn indented(&mut self, write_contents: impl FnOnce(&mut Self)) {
        self.indentation += 1;
        write_contents(self);
        self.indentation -= 1;
    }
}

fn write_quoted(output: &mut String, text: &str) {
    output.push('"');
    for character in text.chars() {
        output.extend(character.escape_default());
    }
    output.push('"');
}

fn write_span(output: &mut String, span: Span) {
    let _ = write!(output, " @{}..{}", span.range().start(), span.range().end());
}
