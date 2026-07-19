//! Deterministic textual rendering of the resolved program.

use std::fmt::Write;

use crate::source::Span;

use super::ir::*;

pub fn dump_resolved(program: &ResolvedProgram) -> String {
    let mut dumper = ResolvedDumper::default();
    dumper.line("ResolvedProgram", program.span);
    dumper.indented(|dumper| {
        dumper.write_indentation();
        match program.entry_function {
            Some(function) => {
                let _ = writeln!(dumper.output, "Entry {}", display_function_id(function));
            }
            None => dumper.output.push_str("Entry <none>\n"),
        }
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
struct ResolvedDumper {
    output: String,
    indentation: usize,
}

impl ResolvedDumper {
    fn function(&mut self, function: &ResolvedFunction) {
        self.write_indentation();
        let _ = write!(
            self.output,
            "Function {} ",
            display_function_id(function.id)
        );
        write_quoted(&mut self.output, &function.name);
        write_span(&mut self.output, function.span);
        self.output.push('\n');

        self.indented(|dumper| {
            dumper.heading("Parameters");
            dumper.indented(|dumper| {
                for parameter in &function.parameters {
                    dumper.write_indentation();
                    let _ = write!(
                        dumper.output,
                        "Parameter {} ",
                        display_parameter_id(parameter.id)
                    );
                    write_quoted(&mut dumper.output, &parameter.name);
                    write_span(&mut dumper.output, parameter.span);
                    dumper.output.push('\n');
                    dumper.indented(|dumper| dumper.type_syntax(&parameter.type_syntax));
                }
            });

            dumper.heading("ReturnType");
            dumper.indented(|dumper| dumper.type_syntax(&function.return_type));

            dumper.heading("Locals");
            dumper.indented(|dumper| {
                for local in &function.locals {
                    dumper.write_indentation();
                    let _ = write!(dumper.output, "Local {} ", display_local_id(local.id));
                    write_quoted(&mut dumper.output, &local.name);
                    write_span(&mut dumper.output, local.span);
                    dumper.output.push('\n');
                    dumper.indented(|dumper| dumper.type_syntax(&local.type_syntax));
                }
            });

            dumper.block(&function.body);
        });
    }

    fn type_syntax(&mut self, type_syntax: &ResolvedType) {
        let name = match type_syntax.kind {
            ResolvedTypeKind::I64 => "I64",
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
                self.line(
                    &format!("LocalDeclaration {}", display_local_id(local.local)),
                    local.span,
                );
                self.indented(|dumper| dumper.expression(&local.initializer));
            }
            ResolvedStatement::Return(statement) => {
                self.line("Return", statement.span);
                self.indented(|dumper| dumper.expression(&statement.value));
            }
            ResolvedStatement::Block(block) => self.block(block),
        }
    }

    fn expression(&mut self, expression: &ResolvedExpression) {
        match expression {
            ResolvedExpression::Binding(binding) => {
                let id = match binding.binding {
                    BindingId::Parameter(id) => display_parameter_id(id),
                    BindingId::Local(id) => display_local_id(id),
                };
                self.line(&format!("Binding {id}"), binding.span);
            }
            ResolvedExpression::Integer(integer) => {
                self.write_indentation();
                self.output.push_str("Integer ");
                write_quoted(&mut self.output, &integer.spelling);
                write_span(&mut self.output, integer.span);
                self.output.push('\n');
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
                self.line(
                    &format!("DirectCall {}", display_function_id(call.function)),
                    call.span,
                );
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

fn display_function_id(id: FunctionId) -> String {
    format!("f{}", id.index())
}

fn display_parameter_id(id: ParameterId) -> String {
    format!("f{}:p{}", id.function().index(), id.index())
}

fn display_local_id(id: LocalId) -> String {
    format!("f{}:l{}", id.function().index(), id.index())
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
