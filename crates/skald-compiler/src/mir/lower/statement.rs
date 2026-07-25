//! Exhaustive statement dispatch and statement-family lowering.

use super::*;
use crate::hir::{HirBlock, HirStatement};

impl BodyLowerer<'_> {
    pub(super) fn lower_block(&mut self, block: &HirBlock) {
        self.cleanup.enter_scope();
        for statement in &block.statements {
            if self.body.is_current_terminated() {
                break;
            }
            debug_assert!(
                self.full_expression_temporaries.is_empty(),
                "a source statement must begin outside any previous full expression"
            );
            self.lower_statement(statement);
        }
        if !self.body.is_current_terminated() {
            self.emit_cleanups(self.cleanup.for_current_scope(block.span));
        }
        self.cleanup.leave_scope();
    }

    fn lower_statement(&mut self, statement: &HirStatement) {
        match statement {
            HirStatement::BaseInitialization(statement) => {
                self.lower_base_initialization(statement)
            }
            HirStatement::Local(local) => self.lower_local(local),
            HirStatement::Return(statement) => self.lower_return(statement),
            HirStatement::Call(statement) => self.lower_call_statement(statement),
            HirStatement::Conditional(conditional) => self.lower_conditional(conditional),
            HirStatement::Block(block) => self.lower_block(block),
            HirStatement::FieldAssignment(assignment) => self.lower_field_assignment(assignment),
            HirStatement::FieldConstruction(statement) => self.lower_field_construction(statement),
            HirStatement::FieldCopyConstruction(statement) => {
                self.lower_field_copy_construction(statement)
            }
            HirStatement::FieldCopyAssignment(statement) => {
                self.lower_field_copy_assignment(statement)
            }
            HirStatement::CopyAssignment(statement) => self.lower_copy_assignment(statement),
            HirStatement::SharedAssignment(assignment) => {
                self.lower_shared_assignment(assignment);
                self.finish_full_expression(assignment.span);
            }
            HirStatement::SharedFieldWrite(write) => {
                self.lower_shared_field_write(write);
                self.finish_full_expression(write.span);
            }
            HirStatement::OptionalAssignment(assignment) => {
                self.lower_optional_assignment(assignment);
                self.finish_full_expression(assignment.span);
            }
        }
    }

    fn lower_base_initialization(&mut self, statement: &crate::hir::HirBaseInitialization) {
        let receiver = self
            .receiver_storage
            .expect("base initialization requires initializer receiver storage");
        let arguments = self.lower_call_arguments(&statement.arguments);
        self.emit(MirInstruction::Initialize(MirInitialize {
            destination: MirPlace::base(receiver).project_base(statement.base),
            target: statement.initializer,
            arguments,
            span: statement.span,
        }));
        self.finish_full_expression(statement.span);
    }

    fn lower_local(&mut self, local: &crate::hir::HirLocalDecl) {
        let storage = self.local_storage[local.local.index()];
        match &local.initializer {
            crate::hir::HirLocalInitializer::Value(initializer) => {
                let value = self
                    .lower_expression(initializer)
                    .expect("typed scalar local initializer must produce a value");
                self.emit(MirInstruction::Store(MirStore {
                    destination: storage.into(),
                    value,
                    span: local.span,
                }));
                self.finish_full_expression(local.span);
            }
            crate::hir::HirLocalInitializer::Object(initialization) => {
                let destination = self.lower_object_place(&initialization.destination);
                self.lower_object_producer(&initialization.producer, destination);
                self.cleanup
                    .register_owned(storage, initialization.producer.class());
                self.finish_full_expression(local.span);
            }
            crate::hir::HirLocalInitializer::Copy(copy) => {
                let source = self.lower_object_source(&copy.source);
                self.emit(MirInstruction::CopyConstruct(MirCopyConstruction {
                    destination: self.lower_object_place(&copy.destination),
                    source,
                    class: copy.destination.class(),
                    operation: lower_selected_copy_operation(copy.operation),
                    span: copy.span,
                }));
                self.cleanup
                    .register_owned(storage, copy.destination.class());
                self.finish_full_expression(local.span);
            }
            crate::hir::HirLocalInitializer::Shared(transfer) => {
                self.lower_shared_local(storage, transfer);
                self.cleanup.register_shared(storage);
                self.finish_full_expression(local.span);
            }
            crate::hir::HirLocalInitializer::Optional(source) => {
                self.lower_optional_initialize(storage, source, local.span);
                self.finish_full_expression(local.span);
            }
        }
    }

    fn lower_return(&mut self, statement: &crate::hir::HirReturn) {
        let value = match &statement.value {
            Some(crate::hir::HirReturnValue::Scalar(value)) => Some(
                self.lower_expression(value)
                    .expect("typed return expression must produce a scalar value"),
            ),
            Some(crate::hir::HirReturnValue::Object(crate::hir::HirObjectReturn::Copy {
                source,
                operation,
                class,
                span,
            })) => {
                let destination = MirPlace::base(
                    self.return_storage
                        .expect("object-returning body must have return storage"),
                );
                let source = self.lower_object_source(source);
                self.emit(MirInstruction::CopyConstruct(MirCopyConstruction {
                    destination,
                    source,
                    class: *class,
                    operation: lower_selected_copy_operation(*operation),
                    span: *span,
                }));
                None
            }
            Some(crate::hir::HirReturnValue::Object(crate::hir::HirObjectReturn::Construct {
                construction,
                ..
            })) => {
                let destination = MirPlace::base(
                    self.return_storage
                        .expect("object-returning body must have return storage"),
                );
                self.lower_construction(construction, destination);
                None
            }
            Some(crate::hir::HirReturnValue::Shared(transfer)) => {
                let destination = self
                    .return_storage
                    .expect("shared-returning body must have return storage");
                self.lower_shared_transfer(destination, transfer);
                self.finish_full_expression(statement.span);
                self.emit_cleanups(self.cleanup.for_all_scopes(statement.span));
                self.terminate(MirTerminator::ReturnShared {
                    owner: destination,
                    span: statement.span,
                });
                return;
            }
            None => None,
        };
        self.finish_full_expression(statement.span);
        self.emit_cleanups(self.cleanup.for_all_scopes(statement.span));
        self.terminate(MirTerminator::Return {
            value,
            span: statement.span,
        });
    }

    fn lower_call_statement(&mut self, statement: &crate::hir::HirCallStatement) {
        let result = self.lower_expression(&statement.call);
        assert!(result.is_none(), "typed call statement must return unit");
        self.finish_full_expression(statement.span);
    }

    fn lower_field_assignment(&mut self, assignment: &crate::hir::HirFieldAssignment) {
        // Receiver selection precedes value evaluation. Current HIR places are
        // stable and therefore emit no instructions while being selected.
        let destination = self.lower_field_place(&assignment.place);
        let value = self
            .lower_expression(&assignment.value)
            .expect("typed field assignment must produce a scalar value");
        self.emit(MirInstruction::Store(MirStore {
            destination,
            value,
            span: assignment.span,
        }));
        self.finish_full_expression(assignment.span);
    }

    fn lower_field_construction(&mut self, statement: &crate::hir::HirFieldConstruction) {
        let destination = self.lower_field_place(&statement.place);
        self.lower_construction_at(&statement.construction, destination, statement.span);
        self.finish_full_expression(statement.span);
    }

    fn lower_field_copy_construction(&mut self, statement: &crate::hir::HirFieldCopyConstruction) {
        let destination = self.lower_field_place(&statement.place);
        let source = self.lower_object_source(&statement.source);
        self.emit(MirInstruction::CopyConstruct(MirCopyConstruction {
            destination,
            source,
            class: statement.source.class(),
            operation: lower_selected_copy_operation(statement.operation),
            span: statement.span,
        }));
        self.finish_full_expression(statement.span);
    }

    fn lower_field_copy_assignment(&mut self, statement: &crate::hir::HirFieldCopyAssignment) {
        let destination = self.lower_field_place(&statement.place);
        let source = self.lower_object_source(&statement.source);
        self.emit(MirInstruction::CopyAssign(MirCopyAssignment {
            destination,
            source,
            class: statement.source.class(),
            operation: lower_selected_copy_operation(statement.operation),
            span: statement.span,
        }));
        self.finish_full_expression(statement.span);
    }

    fn lower_copy_assignment(&mut self, statement: &crate::hir::HirCopyAssignment) {
        let source = self.lower_object_source(&statement.source);
        self.emit(MirInstruction::CopyAssign(MirCopyAssignment {
            destination: self.lower_object_place(&statement.destination),
            source,
            class: statement.destination.class(),
            operation: lower_selected_copy_operation(statement.operation),
            span: statement.span,
        }));
        self.finish_full_expression(statement.span);
    }
}
