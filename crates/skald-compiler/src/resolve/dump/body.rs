//! Function locals, blocks, statements, and control flow.

use std::fmt::Write;

use crate::dump_format::{write_quoted, write_span};

use super::super::ir::*;
use super::ResolvedDumper;

impl ResolvedDumper<'_> {
    pub(super) fn locals(&mut self, locals: &[ResolvedLocal]) {
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

    pub(super) fn block(&mut self, block: &ResolvedBlock) {
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
            ResolvedStatement::Break(statement) => {
                self.line(&format!("Break {}", statement.target), statement.span);
            }
            ResolvedStatement::Continue(statement) => {
                self.line(&format!("Continue {}", statement.target), statement.span);
            }
            ResolvedStatement::Expression(statement) => {
                self.line("ExpressionStatement", statement.span);
                self.indented(|dumper| dumper.expression(&statement.expression));
            }
            ResolvedStatement::Conditional(statement) => self.conditional(statement),
            ResolvedStatement::While(statement) => {
                self.line(&format!("While {}", statement.loop_id), statement.span);
                self.indented(|dumper| {
                    dumper.heading("Condition");
                    dumper.indented(|dumper| dumper.expression(&statement.condition));
                    dumper.block(&statement.body);
                });
            }
            ResolvedStatement::ForIn(statement) => {
                self.line(
                    &format!("ForIn {} {}", statement.loop_id, statement.binding),
                    statement.span,
                );
                self.indented(|dumper| {
                    dumper.line("For", statement.for_span);
                    dumper.line("Binding", statement.binding_span);
                    if let Some(span) = statement.annotation_span {
                        dumper.line("Annotation", span);
                    }
                    dumper.line("In", statement.in_span);
                    dumper.line(
                        &format!(
                            "Selection interface {} item {} state {} iter_state {} iter_next {}",
                            statement.selection.interface,
                            dumper.render_type_kind(statement.selection.item),
                            dumper.render_type_kind(statement.selection.state),
                            statement.selection.iter_state,
                            statement.selection.iter_next,
                        ),
                        statement.selection.origin_span,
                    );
                    match &statement.source {
                        ResolvedForInSource::Iterable(iterable) => {
                            dumper.heading("Iterable");
                            dumper.indented(|dumper| dumper.expression(iterable));
                        }
                        ResolvedForInSource::Range(range) => {
                            let realization = |evidence: ResolvedRangeProtocolEvidence| match evidence.realization {
                                ResolvedRangeProtocolRealization::ClassWitness => "class-witness".to_owned(),
                                ResolvedRangeProtocolRealization::PrimitiveIntrinsic(primitive) => format!("primitive-{}", primitive.name()),
                            };
                            dumper.line(
                                &format!(
                                    "RangeSource template {} class {} initializer {} endpoint {} provenance lower={} upper={} iterable {}",
                                    range.range_template,
                                    range.range_class,
                                    range.initializer,
                                    dumper.render_type_kind(range.endpoint_type),
                                    range.endpoint_provenance[0].name(),
                                    range.endpoint_provenance[1].name(),
                                    range.iterable,
                                ),
                                range.span,
                            );
                            dumper.indented(|dumper| {
                                dumper.line(
                                    &format!("Ordering interface {} requirement {} realization {}", range.ordering.interface, range.ordering.requirement, realization(range.ordering)),
                                    range.operator_span,
                                );
                                dumper.line(
                                    &format!("Successor interface {} requirement {} realization {}", range.successor.interface, range.successor.requirement, realization(range.successor)),
                                    range.operator_span,
                                );
                                dumper.heading("Lower");
                                dumper.indented(|dumper| dumper.expression(&range.lower));
                                dumper.heading("Upper");
                                dumper.indented(|dumper| dumper.expression(&range.upper));
                            });
                        }
                    }
                    dumper.block(&statement.body);
                });
            }
            ResolvedStatement::Block(block) => self.block(block),
            ResolvedStatement::ScalarBindingAssignment(assignment) => {
                self.line(
                    &format!("ScalarBindingAssignment {}", assignment.destination),
                    assignment.span,
                );
                self.indented(|dumper| {
                    dumper.line("Equal", assignment.equal_span);
                    dumper.expression(&assignment.source);
                });
            }
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
            ResolvedStatement::StaticFieldAssignment(assignment) => {
                self.line(
                    &format!("StaticFieldAssignment {}", assignment.field),
                    assignment.span,
                );
                self.indented(|dumper| {
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
            ResolvedStatement::SharedAssignment(assignment) => {
                self.line(
                    &format!("SharedAssignment {}", assignment.destination),
                    assignment.span,
                );
                self.indented(|dumper| {
                    dumper.line("Equal", assignment.equal_span);
                    dumper.expression(&assignment.source);
                });
            }
            ResolvedStatement::OptionalAssignment(assignment) => {
                self.line(
                    &format!(
                        "OptionalAssignment {} type {}",
                        assignment.destination,
                        self.render_type_kind(assignment.target)
                    ),
                    assignment.span,
                );
                self.indented(|dumper| {
                    dumper.line("Equal", assignment.equal_span);
                    dumper.expression(&assignment.source);
                });
            }
            ResolvedStatement::ArrayAssignment(assignment) => {
                self.line("ArrayAssignment", assignment.span);
                self.indented(|dumper| {
                    dumper.heading("Destination");
                    dumper.indented(|dumper| dumper.expression(&assignment.destination));
                    dumper.line("Equal", assignment.equal_span);
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
}
