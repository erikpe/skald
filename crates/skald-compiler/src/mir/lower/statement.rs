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
                !self.full_expression.has_temporaries(),
                "a source statement must begin outside any previous full expression"
            );
            self.lower_statement(statement);
        }
        if !self.body.is_current_terminated() {
            self.emit_scope_exit(self.cleanup.for_current_scope(block.span));
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
            HirStatement::Break(statement) => self.lower_break(statement),
            HirStatement::Continue(statement) => self.lower_continue(statement),
            HirStatement::Panic(statement) => self.lower_panic(statement),
            HirStatement::Call(statement) => self.lower_call_statement(statement),
            HirStatement::Conditional(conditional) => self.lower_conditional(conditional),
            HirStatement::While(statement) => self.lower_while(statement),
            HirStatement::ForIn(statement) => self.lower_for_in(statement),
            HirStatement::Block(block) => self.lower_block(block),
            HirStatement::ScalarAssignment(assignment) => {
                let value = self
                    .lower_expression(&assignment.source)
                    .expect("typed primitive binding assignment must produce a scalar value");
                self.emit(MirInstruction::Store(MirStore {
                    destination: match assignment.destination.storage {
                        crate::hir::HirScalarStorage::Binding(binding) => {
                            self.lower_binding_place(binding)
                        }
                        crate::hir::HirScalarStorage::Static(place) => {
                            MirPlace::static_field(place.field)
                        }
                    },
                    value,
                    authorization: None,
                    final_authorization: None,
                    span: assignment.span,
                }));
                self.finish_full_expression(assignment.span);
            }
            HirStatement::FieldAssignment(assignment) => self.lower_field_assignment(assignment),
            HirStatement::FieldConstruction(statement) => self.lower_field_construction(statement),
            HirStatement::FieldCopyConstruction(statement) => {
                self.lower_field_copy_construction(statement)
            }
            HirStatement::FieldCopyAssignment(statement) => {
                self.lower_field_copy_assignment(statement)
            }
            HirStatement::CopyAssignment(statement) => self.lower_copy_assignment(statement),
            HirStatement::StaticCopyAssignment(statement) => {
                self.lower_static_copy_assignment(statement)
            }
            HirStatement::SharedAssignment(assignment) => {
                self.lower_shared_assignment(assignment);
                self.finish_full_expression(assignment.span);
            }
            HirStatement::SharedFieldWrite(write) => {
                self.lower_shared_field_write(write);
                self.finish_full_expression(write.span);
            }
            HirStatement::SharedStaticAssignment(assignment) => {
                self.lower_shared_static_assignment(assignment);
                self.finish_full_expression(assignment.span);
            }
            HirStatement::OptionalAssignment(assignment) => {
                self.lower_optional_assignment(assignment);
                self.finish_full_expression(assignment.span);
            }
            HirStatement::ClassOptionalAssignment(assignment) => {
                self.lower_class_optional_assignment(assignment);
                self.finish_full_expression(assignment.span);
            }
            HirStatement::OptionalSharedAssignment(assignment) => {
                self.lower_optional_shared_assignment(assignment);
                self.finish_full_expression(assignment.span);
            }
            HirStatement::AggregateOptionalAssignment(assignment) => {
                self.lower_aggregate_optional_assignment(assignment);
                self.finish_full_expression(assignment.span);
            }
            HirStatement::ArrayFieldInitialize(initialize) => {
                let destination = self.lower_field_place(&initialize.place);
                self.lower_array_initialize(destination, &initialize.value, false);
                self.finish_full_expression(initialize.span);
            }
            HirStatement::ArrayAssignment(assignment) => {
                let destination = self.lower_array_place(&assignment.destination);
                self.lower_array_replace(
                    destination,
                    &assignment.value,
                    super::lower_array_cell_write_authorization(&assignment.destination),
                    super::lower_array_final_write_authorization(&assignment.destination),
                );
                self.finish_full_expression(assignment.span);
            }
            HirStatement::ArrayElementAssignment(assignment) => {
                self.lower_array_element_assignment(assignment);
                self.finish_full_expression(assignment.span);
            }
            HirStatement::ArraySliceAssignment(assignment) => {
                self.lower_array_slice_assignment(assignment);
                self.finish_full_expression(assignment.span);
            }
        }
    }

    fn lower_panic(&mut self, statement: &crate::hir::HirPanic) {
        let argument = crate::hir::HirCallArgument::Copy(statement.message.clone());
        let mut arguments = self.lower_call_arguments(std::slice::from_ref(&argument));
        let message = match arguments.pop() {
            Some(MirArgument::OwnedPlace(message)) if arguments.is_empty() => message,
            _ => unreachable!("checked panic message must lower to one owned string place"),
        };
        self.terminate(MirTerminator::Panic {
            message,
            span: statement.span,
        });
        self.discard_terminated_full_expression_tracking();
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
        self.begin_storage_lifetime(storage, local.span);
        self.cleanup.register_storage(storage);
        match &local.initializer {
            crate::hir::HirLocalInitializer::Value(initializer) => {
                let value = self
                    .lower_expression(initializer)
                    .expect("typed scalar local initializer must produce a value");
                self.emit(MirInstruction::Store(MirStore {
                    destination: storage.into(),
                    value,
                    authorization: None,
                    final_authorization: None,
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
                let optional_mark = self.optional_view_mark();
                let source = self.lower_object_source(&copy.source);
                self.emit(MirInstruction::CopyConstruct(MirCopyConstruction {
                    destination: self.lower_object_place(&copy.destination),
                    source,
                    class: copy.destination.class(),
                    operation: lower_selected_copy_operation(copy.operation),
                    span: copy.span,
                }));
                self.end_optional_views_from(optional_mark, copy.span);
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
            crate::hir::HirLocalInitializer::ClassOptional(value) => {
                self.lower_class_optional_initialize(storage, value);
                self.cleanup.register_class_optional(
                    storage,
                    optional_types::class_id(self.input.optional_types, value.class),
                    value.class,
                );
                self.finish_full_expression(local.span);
            }
            crate::hir::HirLocalInitializer::OptionalShared(value) => {
                self.lower_optional_shared_initialize(storage, value);
                self.cleanup.register_optional_shared(
                    storage,
                    optional_types::shared_id(self.input.optional_types, value.target),
                    super::lower_shared_target(value.target),
                );
                self.finish_full_expression(local.span);
            }
            crate::hir::HirLocalInitializer::AggregateOptional(value) => {
                self.lower_aggregate_optional_initialize_at(MirPlace::base(storage), value);
                self.cleanup
                    .register_aggregate_optional(storage, value.optional);
                self.finish_full_expression(local.span);
            }
            crate::hir::HirLocalInitializer::Array(initialization) => {
                self.lower_array_initialize(MirPlace::base(storage), initialization, false);
                self.cleanup
                    .register_array(storage, initialization.source.array);
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
            Some(crate::hir::HirReturnValue::Optional(source)) => {
                let destination = self
                    .return_storage
                    .expect("optional-returning body must have return storage");
                let source = self.lower_optional_source(source);
                self.emit(MirInstruction::OptionalInitialize(MirOptionalInitialize {
                    destination: MirPlace::base(destination),
                    source,
                    span: statement.span,
                }));
                None
            }
            Some(crate::hir::HirReturnValue::ClassOptional(value)) => {
                let destination = self
                    .return_storage
                    .expect("class-optional-returning body must have return storage");
                self.lower_class_optional_initialize(destination, value);
                None
            }
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
                let optional_mark = self.optional_view_mark();
                let source = self.lower_object_source(source);
                self.emit(MirInstruction::CopyConstruct(MirCopyConstruction {
                    destination,
                    source,
                    class: *class,
                    operation: lower_selected_copy_operation(*operation),
                    span: *span,
                }));
                self.end_optional_views_from(optional_mark, *span);
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
                self.emit_scope_exit(self.cleanup.for_all_scopes(statement.span));
                self.terminate(MirTerminator::ReturnShared {
                    owner: destination,
                    span: statement.span,
                });
                return;
            }
            Some(crate::hir::HirReturnValue::OptionalShared(value)) => {
                let destination = self
                    .return_storage
                    .expect("optional-shared-returning body must have return storage");
                self.lower_optional_shared_initialize(destination, value);
                self.finish_full_expression(statement.span);
                self.emit_scope_exit(self.cleanup.for_all_scopes(statement.span));
                self.terminate(MirTerminator::ReturnOptionalShared {
                    owner: destination,
                    span: statement.span,
                });
                return;
            }
            Some(crate::hir::HirReturnValue::AggregateOptional(value)) => {
                let destination = self
                    .return_storage
                    .expect("aggregate-optional-returning body must have return storage");
                self.lower_aggregate_optional_initialize_at(MirPlace::base(destination), value);
                None
            }
            Some(crate::hir::HirReturnValue::Array(initialization)) => {
                let destination = MirPlace::base(
                    self.return_storage
                        .expect("array-returning body must have return storage"),
                );
                self.lower_array_initialize(destination, initialization, false);
                None
            }
            None => None,
        };
        let scope_exit = self.cleanup.for_all_scopes(statement.span);
        let preserve_scalar =
            scope_exit.requires_optional_check() || self.full_expression.cleanup_may_change_block();
        let spilled_value = value.filter(|_| preserve_scalar).map(|value| {
            self.spill_scalar(
                value,
                self.lower_type(self.input.return_type),
                statement.span,
            )
        });
        if let Some((storage, _)) = spilled_value {
            self.extend_storage_beyond_full_expression(storage);
        }
        self.finish_full_expression(statement.span);
        self.emit_scope_exit(scope_exit);
        let value = spilled_value
            .map(|(storage, ty)| {
                let value = self.assign(MirRvalueKind::Load(storage.into()), ty, statement.span);
                self.end_storage_lifetime(storage, statement.span);
                value
            })
            .or(value);
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
            authorization: super::lower_cell_write_authorization(&assignment.place),
            final_authorization: super::lower_final_write_authorization(&assignment.place),
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
        let optional_mark = self.optional_view_mark();
        let source = self.lower_object_source(&statement.source);
        self.emit(MirInstruction::CopyConstruct(MirCopyConstruction {
            destination,
            source,
            class: statement.source.class(),
            operation: lower_selected_copy_operation(statement.operation),
            span: statement.span,
        }));
        self.end_optional_views_from(optional_mark, statement.span);
        self.finish_full_expression(statement.span);
    }

    fn lower_field_copy_assignment(&mut self, statement: &crate::hir::HirFieldCopyAssignment) {
        let destination = self.lower_field_place(&statement.place);
        let optional_mark = self.optional_view_mark();
        let source = self.lower_object_source(&statement.source);
        self.emit(MirInstruction::CopyAssign(MirCopyAssignment {
            destination,
            source,
            class: statement.source.class(),
            operation: lower_selected_copy_operation(statement.operation),
            authorization: super::lower_cell_write_authorization(&statement.place),
            final_authorization: super::lower_final_write_authorization(&statement.place),
            span: statement.span,
        }));
        self.end_optional_views_from(optional_mark, statement.span);
        self.finish_full_expression(statement.span);
    }

    fn lower_copy_assignment(&mut self, statement: &crate::hir::HirCopyAssignment) {
        let optional_mark = self.optional_view_mark();
        let source = self.lower_object_source(&statement.source);
        self.emit(MirInstruction::CopyAssign(MirCopyAssignment {
            destination: self.lower_object_place(&statement.destination),
            source,
            class: statement.destination.class(),
            operation: lower_selected_copy_operation(statement.operation),
            authorization: None,
            final_authorization: None,
            span: statement.span,
        }));
        self.end_optional_views_from(optional_mark, statement.span);
        self.finish_full_expression(statement.span);
    }

    fn lower_static_copy_assignment(&mut self, statement: &crate::hir::HirStaticCopyAssignment) {
        let optional_mark = self.optional_view_mark();
        let source = self.lower_object_source(&statement.source);
        self.emit(MirInstruction::CopyAssign(MirCopyAssignment {
            destination: MirPlace::static_field(statement.destination.field),
            source,
            class: statement.class,
            operation: lower_selected_copy_operation(statement.operation),
            authorization: None,
            final_authorization: None,
            span: statement.span,
        }));
        self.end_optional_views_from(optional_mark, statement.span);
        self.finish_full_expression(statement.span);
    }
}
