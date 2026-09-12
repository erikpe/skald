//! Rendering for HIR locals, blocks, and statements.

use std::fmt::Write;

use crate::dump_format::{write_quoted, write_span};

use super::super::ir::*;
use super::aggregate::array_assignment_name;
use super::ownership::optional_shared_target_name;
use super::HirDumper;

impl<'types> HirDumper<'types> {
    pub(super) fn locals(&mut self, locals: &[HirLocal]) {
        self.heading("Locals");
        self.indented(|dumper| {
            for local in locals {
                dumper.local(local, "Local");
            }
        });
    }

    pub(super) fn local(&mut self, local: &HirLocal, label: &str) {
        self.write_indentation();
        let _ = write!(self.output, "{label} {} ", local.id);
        write_quoted(&mut self.output, &local.name);
        let _ = write!(self.output, " : {}", self.type_name(local.ty));
        write_span(&mut self.output, local.span);
        self.output.push('\n');
    }

    pub(super) fn block(&mut self, block: &HirBlock) {
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
                    HirLocalInitializer::OptionalShared(value) => {
                        dumper.optional_shared_value(value)
                    }
                    HirLocalInitializer::AggregateOptional(value) => {
                        dumper.aggregate_optional_value(value)
                    }
                    HirLocalInitializer::Array(value) => dumper.array_initialize(value),
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
                        HirReturnValue::OptionalShared(value) => {
                            dumper.optional_shared_value(value)
                        }
                        HirReturnValue::AggregateOptional(value) => {
                            dumper.aggregate_optional_value(value)
                        }
                        HirReturnValue::Array(value) => dumper.array_initialize(value),
                    });
                }
            }
            HirStatement::Break(statement) => {
                self.line(&format!("Break {}", statement.target), statement.span);
            }
            HirStatement::Continue(statement) => {
                self.line(&format!("Continue {}", statement.target), statement.span);
            }
            HirStatement::Call(statement) => {
                self.line("CallStatement", statement.span);
                self.indented(|dumper| dumper.expression(&statement.call));
            }
            HirStatement::Panic(statement) => {
                self.line("Panic", statement.span);
                self.indented(|dumper| dumper.object_source(&statement.message.source));
            }
            HirStatement::Conditional(statement) => self.conditional(statement),
            HirStatement::While(statement) => {
                self.line(&format!("While {}", statement.loop_id), statement.span);
                self.indented(|dumper| {
                    dumper.line("Condition", statement.condition.span);
                    dumper.indented(|dumper| dumper.expression(&statement.condition));
                    dumper.block(&statement.body);
                });
            }
            HirStatement::ForIn(statement) => self.for_in(statement),
            HirStatement::Block(block) => self.block(block),
            HirStatement::ScalarAssignment(assignment) => {
                match assignment.destination.storage {
                    crate::hir::HirScalarStorage::Binding(binding) => self.line(
                        &format!("ScalarBindingAssignment {binding}"),
                        assignment.span,
                    ),
                    crate::hir::HirScalarStorage::Static(place) => self.line(
                        &format!("ScalarStaticAssignment {}", place.field),
                        assignment.span,
                    ),
                }
                self.indented(|dumper| dumper.expression(&assignment.source));
            }
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
            HirStatement::StaticCopyAssignment(statement) => {
                self.line(
                    &format!(
                        "StaticCopyAssignment {} : class {}",
                        statement.destination.field, statement.class
                    ),
                    statement.span,
                );
                self.indented(|dumper| {
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
            HirStatement::SharedStaticAssignment(statement) => {
                self.line("SharedStaticAssignment", statement.span);
                self.indented(|dumper| {
                    dumper.raw_line(&format!("StaticField {}", statement.destination.field));
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
                    dumper.class_optional_place(&assignment.destination);
                    dumper.class_optional_source(&assignment.source);
                });
            }
            HirStatement::OptionalSharedAssignment(assignment) => {
                self.line(
                    &format!(
                        "OptionalSharedAssignment {}",
                        optional_shared_target_name(assignment.destination.target)
                    ),
                    assignment.span,
                );
                self.indented(|dumper| {
                    dumper.optional_shared_place(&assignment.destination);
                    dumper.optional_shared_source(&assignment.source);
                });
            }
            HirStatement::AggregateOptionalAssignment(assignment) => {
                self.line(
                    &format!("AggregateOptionalAssignment {}", assignment.value.optional),
                    assignment.span,
                );
                self.indented(|dumper| dumper.aggregate_optional_value(&assignment.value));
            }
            HirStatement::ArrayFieldInitialize(statement) => {
                self.line("ArrayFieldInitialization", statement.span);
                self.indented(|dumper| {
                    dumper.field_place(&statement.place);
                    dumper.array_initialize(&statement.value);
                });
            }
            HirStatement::ArrayAssignment(statement) => {
                self.line(
                    &format!("ArrayReplacement {:?}", statement.evaluation),
                    statement.span,
                );
                self.indented(|dumper| {
                    dumper.array_place(&statement.destination);
                    dumper.array_initialize(&statement.value);
                });
            }
            HirStatement::ArrayElementAssignment(statement) => {
                self.line(
                    &format!(
                        "ArrayElementAssignment {} {:?}",
                        array_assignment_name(statement.operation),
                        statement.evaluation
                    ),
                    statement.span,
                );
                self.indented(|dumper| {
                    dumper.array_element(&statement.destination);
                    dumper.array_element_value(&statement.value);
                });
            }
            HirStatement::ArraySliceAssignment(statement) => {
                self.line(
                    &format!(
                        "ArraySliceAssignment {} failure={:?} {:?}",
                        array_assignment_name(statement.operation),
                        statement.failure,
                        statement.evaluation
                    ),
                    statement.span,
                );
                self.indented(|dumper| {
                    dumper.array_slice(&statement.destination);
                    dumper.array_source(&statement.source);
                });
            }
        }
    }
}
