//! Stable, source-independent textual rendering of the source AST.

use std::fmt::Write;

use crate::source::Span;

use super::ast::*;

pub fn dump_ast(ast: &CompilationUnit) -> String {
    let mut dumper = AstDumper::default();
    dumper.line("CompilationUnit", ast.span);
    dumper.indented(|dumper| {
        for function in &ast.functions {
            dumper.function(function);
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
    fn function(&mut self, function: &FunctionDecl) {
        self.line("Function", function.span);
        self.indented(|dumper| {
            dumper.named("Name", &function.name.text, function.name.span);
            dumper.heading("Parameters");
            dumper.indented(|dumper| {
                for parameter in &function.parameters {
                    dumper.line("Parameter", parameter.span);
                    dumper.indented(|dumper| {
                        dumper.named("Name", &parameter.name.text, parameter.name.span);
                        dumper.type_syntax(&parameter.type_syntax);
                    });
                }
            });
            dumper.heading("ReturnType");
            dumper.indented(|dumper| dumper.type_syntax(&function.return_type));
            dumper.block(&function.body);
        });
    }

    fn type_syntax(&mut self, type_syntax: &TypeSyntax) {
        let kind = match type_syntax.kind {
            TypeKind::I64 => "I64",
            TypeKind::Unit => "Unit",
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
            Statement::Block(block) => self.block(block),
        }
    }

    fn expression(&mut self, expression: &Expression) {
        match expression {
            Expression::Identifier(identifier) => {
                self.named("Identifier", &identifier.name.text, identifier.span);
            }
            Expression::Integer(integer) => {
                self.named("Integer", &integer.spelling, integer.span);
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
        }
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
