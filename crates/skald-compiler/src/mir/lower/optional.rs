//! Primitive optional storage and checked-access lowering.

use crate::{
    hir::{
        HirClassOptionalAssignment, HirClassOptionalInitialize, HirClassOptionalPlace,
        HirClassOptionalSource, HirObjectSource, HirOptionalOperand, HirOptionalPlace,
        HirOptionalSource, HirOptionalStorage, HirPresenceTestKind, Type,
    },
    mir::{
        MirClassOptionalAssign, MirClassOptionalInitialize, MirClassOptionalPublish,
        MirClassOptionalSource, MirInstruction, MirOptionalAssign, MirOptionalInitialize,
        MirOptionalSource, MirPresenceTestKind, MirPrimitiveType, MirRvalueKind, MirStorage,
        MirStorageKind, MirTerminationReason, MirTerminator, MirType, StorageId,
    },
};

use super::{array_lowering_gate, BodyLowerer};

impl BodyLowerer<'_> {
    pub(super) fn lower_class_optional_initialize(
        &mut self,
        destination: StorageId,
        value: &HirClassOptionalInitialize,
    ) {
        self.lower_class_optional_initialize_at(crate::mir::MirPlace::base(destination), value);
    }

    fn lower_class_optional_initialize_at(
        &mut self,
        destination: crate::mir::MirPlace,
        value: &HirClassOptionalInitialize,
    ) {
        let optional_mark = self.optional_view_mark();
        match &value.source {
            HirClassOptionalSource::Produced(expression) => {
                self.lower_optional_call(expression, destination);
            }
            HirClassOptionalSource::Present(HirObjectSource::Produced(producer)) => {
                self.emit(MirInstruction::ClassOptionalInitialize(
                    MirClassOptionalInitialize {
                        destination: destination.clone(),
                        source: MirClassOptionalSource::Absent,
                        class: value.class,
                        copy_constructor: None,
                        span: value.span,
                    },
                ));
                self.lower_object_producer(
                    producer,
                    destination.clone().project_optional_payload(value.class),
                );
                self.emit(MirInstruction::ClassOptionalPublish(
                    MirClassOptionalPublish {
                        destination,
                        class: value.class,
                        span: value.span,
                    },
                ));
            }
            source => {
                let source = self.lower_class_optional_source(source);
                self.emit(MirInstruction::ClassOptionalInitialize(
                    MirClassOptionalInitialize {
                        destination,
                        source,
                        class: value.class,
                        copy_constructor: value
                            .copy_constructor
                            .map(super::lower_selected_copy_operation),
                        span: value.span,
                    },
                ));
            }
        }
        self.end_optional_views_from(optional_mark, value.span);
    }

    pub(super) fn lower_class_optional_assignment(
        &mut self,
        assignment: &HirClassOptionalAssignment,
    ) {
        let destination = self.lower_class_optional_place(&assignment.destination);
        if assignment.kind == crate::hir::HirOptionalWriteKind::Initialize {
            self.lower_class_optional_initialize_at(
                destination,
                &HirClassOptionalInitialize {
                    class: assignment.destination.class,
                    source: assignment.source.clone(),
                    copy_constructor: assignment.copy_constructor,
                    span: assignment.span,
                },
            );
            return;
        }
        let source = match &assignment.source {
            HirClassOptionalSource::Produced(expression) => {
                let storage = self.new_optional_storage(
                    MirStorageKind::Temporary,
                    "class-optional-result",
                    expression.ty,
                    expression.span,
                );
                self.lower_optional_call(expression, crate::mir::MirPlace::base(storage));
                self.full_expression_temporaries.push(
                    super::FullExpressionTemporary::ClassOptional(
                        crate::mir::MirClassOptionalCleanup {
                            destination: crate::mir::MirPlace::base(storage),
                            class: assignment.destination.class,
                            span: expression.span,
                        },
                    ),
                );
                MirClassOptionalSource::Copy(crate::mir::MirPlace::base(storage))
            }
            source => self.lower_class_optional_source(source),
        };
        let self_copy =
            matches!(&source, MirClassOptionalSource::Copy(source) if source == &destination);
        if !self_copy {
            self.check_optional_mutation(destination.clone(), assignment.span);
        }
        self.emit(MirInstruction::ClassOptionalAssign(
            MirClassOptionalAssign {
                destination,
                source,
                class: assignment.destination.class,
                copy_constructor: assignment
                    .copy_constructor
                    .map(super::lower_selected_copy_operation),
                copy_assignment: assignment
                    .copy_assignment
                    .map(super::lower_selected_copy_operation),
                span: assignment.span,
            },
        ));
    }

    pub(super) fn check_optional_mutation(
        &mut self,
        source: crate::mir::MirPlace,
        span: crate::source::Span,
    ) {
        let success_target = self.body.allocate_block(span);
        let failure_target = self.body.allocate_block(span);
        self.terminate(MirTerminator::CheckOptionalMutation {
            source,
            success_target,
            failure_target,
            span,
        });
        self.body
            .select_block(failure_target)
            .expect("allocated optional-mutation failure block must be selectable");
        self.terminate(MirTerminator::Terminate {
            reason: MirTerminationReason::OptionalPinnedMutation,
            span,
        });
        self.body
            .select_block(success_target)
            .expect("allocated optional-mutation success block must be selectable");
    }

    pub(super) fn emit_class_optional_cleanup(
        &mut self,
        cleanup: crate::mir::MirClassOptionalCleanup,
    ) {
        self.check_optional_mutation(cleanup.destination.clone(), cleanup.span);
        self.emit(MirInstruction::ClassOptionalCleanup(cleanup));
    }

    fn lower_class_optional_source(
        &mut self,
        source: &HirClassOptionalSource,
    ) -> MirClassOptionalSource {
        match source {
            HirClassOptionalSource::Absent { .. } => MirClassOptionalSource::Absent,
            HirClassOptionalSource::Present(source) => {
                MirClassOptionalSource::Present(self.lower_object_source(source))
            }
            HirClassOptionalSource::Copy(place) => {
                MirClassOptionalSource::Copy(self.lower_class_optional_place(place))
            }
            HirClassOptionalSource::Produced(_) => {
                unreachable!("produced optional sources are lowered at their consumer")
            }
        }
    }

    pub(super) fn lower_class_optional_place(
        &mut self,
        place: &HirClassOptionalPlace,
    ) -> crate::mir::MirPlace {
        match &place.storage {
            HirOptionalStorage::Binding(binding) => self.lower_binding_place(*binding),
            HirOptionalStorage::Field(field) => self.lower_field_place(field),
            HirOptionalStorage::ArrayElement(_) => array_lowering_gate(),
        }
    }

    pub(super) fn lower_optional_initialize(
        &mut self,
        destination: StorageId,
        source: &HirOptionalSource,
        span: crate::source::Span,
    ) {
        let source = self.lower_optional_source(source);
        self.emit(MirInstruction::OptionalInitialize(MirOptionalInitialize {
            destination: crate::mir::MirPlace::base(destination),
            source,
            span,
        }));
    }

    pub(super) fn lower_optional_assignment(
        &mut self,
        assignment: &crate::hir::HirOptionalAssignment,
    ) {
        let destination = self.lower_optional_place(&assignment.destination);
        let source = self.lower_optional_source(&assignment.source);
        self.emit(match assignment.kind {
            crate::hir::HirOptionalWriteKind::Initialize => {
                MirInstruction::OptionalInitialize(MirOptionalInitialize {
                    destination,
                    source,
                    span: assignment.span,
                })
            }
            crate::hir::HirOptionalWriteKind::Assign => {
                MirInstruction::OptionalAssign(MirOptionalAssign {
                    destination,
                    source,
                    span: assignment.span,
                })
            }
        });
        self.finish_full_expression(assignment.span);
    }

    pub(super) fn lower_presence_test(
        &mut self,
        expression: &crate::hir::HirExpression,
        source: &HirOptionalOperand,
        kind: HirPresenceTestKind,
    ) -> crate::mir::ValueId {
        let source = self.lower_optional_operand(source);
        self.assign(
            MirRvalueKind::OptionalPresence {
                source,
                kind: match kind {
                    HirPresenceTestKind::Some => MirPresenceTestKind::Some,
                    HirPresenceTestKind::None => MirPresenceTestKind::None,
                },
            },
            MirType::Bool,
            expression.span,
        )
    }

    pub(super) fn lower_optional_unwrap(
        &mut self,
        expression: &crate::hir::HirExpression,
        source: &HirOptionalOperand,
    ) -> crate::mir::ValueId {
        let source_storage = self.lower_optional_operand(source);
        let payload = lower_primitive_type(source.payload());
        let destination = StorageId::new(self.input.callable, self.storage.len());
        self.storage.push(MirStorage {
            id: destination,
            source: None,
            name: format!("unwrap{}", destination.index()),
            kind: MirStorageKind::OptionalUnwrap,
            ty: payload.payload_type(),
            span: expression.span,
        });

        let success_target = self.body.allocate_block(expression.span);
        let failure_target = self.body.allocate_block(expression.span);
        self.terminate(MirTerminator::OptionalUnwrap {
            source: source_storage,
            destination,
            success_target,
            failure_target,
            span: expression.span,
        });

        self.body
            .select_block(failure_target)
            .expect("allocated optional failure block must be selectable");
        self.terminate(MirTerminator::Terminate {
            reason: MirTerminationReason::OptionalAccessFailure,
            span: expression.span,
        });

        self.body
            .select_block(success_target)
            .expect("allocated optional success block must be selectable");
        self.assign(
            MirRvalueKind::Load(destination.into()),
            payload.payload_type(),
            expression.span,
        )
    }

    pub(super) fn lower_optional_source(
        &mut self,
        source: &HirOptionalSource,
    ) -> MirOptionalSource {
        match source {
            HirOptionalSource::Absent { .. } => MirOptionalSource::Absent,
            HirOptionalSource::Present(expression) => MirOptionalSource::Present(
                self.lower_expression(expression)
                    .expect("typed primitive optional payload must produce a scalar value"),
            ),
            HirOptionalSource::Copy(place) => {
                MirOptionalSource::Copy(self.lower_optional_place(place))
            }
            HirOptionalSource::Produced(expression) => {
                let destination = self.new_optional_storage(
                    MirStorageKind::Temporary,
                    "optional-result",
                    expression.ty,
                    expression.span,
                );
                self.lower_optional_call(expression, crate::mir::MirPlace::base(destination));
                MirOptionalSource::Copy(crate::mir::MirPlace::base(destination))
            }
        }
    }

    pub(super) fn lower_optional_place(
        &mut self,
        place: &HirOptionalPlace,
    ) -> crate::mir::MirPlace {
        match &place.storage {
            HirOptionalStorage::Binding(binding) => self.lower_binding_place(*binding),
            HirOptionalStorage::Field(field) => self.lower_field_place(field),
            HirOptionalStorage::ArrayElement(_) => array_lowering_gate(),
        }
    }

    pub(super) fn lower_optional_shared_initialize(
        &mut self,
        destination: StorageId,
        value: &crate::hir::HirOptionalSharedInitialize,
    ) {
        let source = self.lower_optional_shared_source(&value.source);
        self.emit(MirInstruction::OptionalSharedInitialize(
            crate::mir::MirOptionalSharedInitialize {
                destination: crate::mir::MirPlace::base(destination),
                source,
                target: super::lower_shared_target(value.target),
                span: value.span,
            },
        ));
    }

    pub(super) fn lower_optional_shared_assignment(
        &mut self,
        assignment: &crate::hir::HirOptionalSharedAssignment,
    ) {
        let source = self.lower_optional_shared_source(&assignment.source);
        let destination = self.lower_optional_shared_place(&assignment.destination);
        let operation = crate::mir::MirOptionalSharedAssign {
            destination,
            source,
            target: super::lower_shared_target(assignment.destination.target),
            span: assignment.span,
        };
        match assignment.kind {
            crate::hir::HirOptionalWriteKind::Initialize => {
                self.emit(MirInstruction::OptionalSharedInitialize(
                    crate::mir::MirOptionalSharedInitialize {
                        destination: operation.destination,
                        source: operation.source,
                        target: operation.target,
                        span: operation.span,
                    },
                ));
            }
            crate::hir::HirOptionalWriteKind::Assign => {
                self.emit(MirInstruction::OptionalSharedAssign(operation));
            }
        }
    }

    fn lower_optional_shared_source(
        &mut self,
        source: &crate::hir::HirOptionalSharedSource,
    ) -> crate::mir::MirOptionalSharedSource {
        match source {
            crate::hir::HirOptionalSharedSource::Absent { .. } => {
                crate::mir::MirOptionalSharedSource::Absent
            }
            crate::hir::HirOptionalSharedSource::Present(source) => {
                let storage = self.new_untracked_shared_storage(source.target(), source.span());
                self.lower_shared_source(storage, source, source.span());
                crate::mir::MirOptionalSharedSource::Present(storage)
            }
            crate::hir::HirOptionalSharedSource::Copy(place) => {
                crate::mir::MirOptionalSharedSource::Copy(self.lower_optional_shared_place(place))
            }
            crate::hir::HirOptionalSharedSource::Produced(expression) => {
                let target = match expression.ty {
                    Type::OptionalShared(target) => target,
                    _ => unreachable!("optional shared producer must have optional shared type"),
                };
                let storage = self.new_optional_storage(
                    MirStorageKind::Temporary,
                    "optional-shared-result",
                    Type::OptionalShared(target),
                    expression.span,
                );
                self.lower_optional_shared_call(expression, storage);
                crate::mir::MirOptionalSharedSource::Move(storage)
            }
        }
    }

    fn lower_optional_shared_place(
        &mut self,
        place: &crate::hir::HirOptionalSharedPlace,
    ) -> crate::mir::MirPlace {
        match &place.storage {
            HirOptionalStorage::Binding(binding) => self.lower_binding_place(*binding),
            HirOptionalStorage::Field(field) => self.lower_field_place(field),
            HirOptionalStorage::ArrayElement(_) => array_lowering_gate(),
        }
    }

    fn new_untracked_shared_storage(
        &mut self,
        target: crate::hir::HirSharedTarget,
        span: crate::source::Span,
    ) -> StorageId {
        let storage = StorageId::new(self.input.callable, self.storage.len());
        self.storage.push(MirStorage {
            id: storage,
            source: None,
            name: format!("optional-shared-source-{}", storage.index()),
            kind: MirStorageKind::Temporary,
            ty: MirType::Shared(super::lower_shared_target(target)),
            span,
        });
        storage
    }

    pub(super) fn lower_optional_shared_unwrap(
        &mut self,
        operand: &HirOptionalOperand,
        destination: StorageId,
    ) {
        let source = self.lower_optional_operand(operand);
        let target = super::lower_shared_target(operand.shared_target());
        let success_target = self.body.allocate_block(operand.span());
        let failure_target = self.body.allocate_block(operand.span());
        self.terminate(MirTerminator::OptionalSharedUnwrap {
            unwrap: crate::mir::MirOptionalSharedUnwrap {
                source,
                destination,
                target,
                span: operand.span(),
            },
            success_target,
            failure_target,
            span: operand.span(),
        });
        self.body
            .select_block(failure_target)
            .expect("allocated optional-owner failure block must be selectable");
        self.terminate(MirTerminator::Terminate {
            reason: MirTerminationReason::OptionalAccessFailure,
            span: operand.span(),
        });
        self.body
            .select_block(success_target)
            .expect("allocated optional-owner success block must be selectable");
    }

    pub(super) fn lower_optional_operand(
        &mut self,
        operand: &HirOptionalOperand,
    ) -> crate::mir::MirPlace {
        match operand {
            HirOptionalOperand::Place(place) => self.lower_optional_place(place),
            HirOptionalOperand::Produced(expression) => {
                let destination = self.new_optional_storage(
                    MirStorageKind::Temporary,
                    "optional-result",
                    expression.ty,
                    expression.span,
                );
                self.lower_optional_call(expression, crate::mir::MirPlace::base(destination));
                crate::mir::MirPlace::base(destination)
            }
            HirOptionalOperand::ClassPlace(place) => self.lower_class_optional_place(place),
            HirOptionalOperand::ClassProduced(expression) => {
                let destination = self.new_optional_storage(
                    MirStorageKind::Temporary,
                    "class-optional-result",
                    expression.ty,
                    expression.span,
                );
                self.lower_optional_call(expression, crate::mir::MirPlace::base(destination));
                self.full_expression_temporaries.push(
                    super::FullExpressionTemporary::ClassOptional(
                        crate::mir::MirClassOptionalCleanup {
                            destination: crate::mir::MirPlace::base(destination),
                            class: match expression.ty {
                                Type::OptionalClass(class) => class,
                                _ => unreachable!(),
                            },
                            span: expression.span,
                        },
                    ),
                );
                crate::mir::MirPlace::base(destination)
            }
            HirOptionalOperand::SharedPlace(place) => self.lower_optional_shared_place(place),
            HirOptionalOperand::SharedProduced(expression) => {
                let target = match expression.ty {
                    Type::OptionalShared(target) => target,
                    _ => unreachable!(),
                };
                let destination = self.new_optional_storage(
                    MirStorageKind::Temporary,
                    "optional-shared-result",
                    expression.ty,
                    expression.span,
                );
                self.lower_optional_shared_call(expression, destination);
                self.full_expression_temporaries.push(
                    super::FullExpressionTemporary::OptionalShared(
                        crate::mir::MirOptionalSharedCleanup {
                            destination: crate::mir::MirPlace::base(destination),
                            target: super::lower_shared_target(target),
                            span: expression.span,
                        },
                    ),
                );
                crate::mir::MirPlace::base(destination)
            }
        }
    }

    pub(super) fn optional_view_mark(&self) -> usize {
        self.active_optional_guards.len()
    }

    pub(super) fn begin_optional_view(
        &mut self,
        view: &crate::hir::HirCheckedOptionalView,
    ) -> crate::mir::MirPlace {
        let source = self.lower_optional_operand(&view.source);
        let class = view.source.class();
        let guard = crate::mir::OptionalGuardId::new(self.input.callable, self.next_optional_guard);
        self.next_optional_guard += 1;
        let success_target = self.body.allocate_block(view.span);
        let absent_target = self.body.allocate_block(view.span);
        let overflow_target = self.body.allocate_block(view.span);
        self.terminate(MirTerminator::BeginOptionalView {
            begin: crate::mir::MirOptionalViewBegin {
                guard,
                source: source.clone(),
                class,
                span: view.span,
            },
            success_target,
            absent_target,
            overflow_target,
            span: view.span,
        });
        self.body
            .select_block(absent_target)
            .expect("allocated optional-view absence block must be selectable");
        self.terminate(MirTerminator::Terminate {
            reason: MirTerminationReason::OptionalAccessFailure,
            span: view.span,
        });
        self.body
            .select_block(overflow_target)
            .expect("allocated optional-view overflow block must be selectable");
        self.terminate(MirTerminator::Terminate {
            reason: MirTerminationReason::OptionalGuardOverflow,
            span: view.span,
        });
        self.body
            .select_block(success_target)
            .expect("allocated optional-view success block must be selectable");
        self.active_optional_guards
            .push(super::ActiveOptionalGuard {
                guard,
                source: source.clone(),
                class,
            });
        source.project_optional_payload(class)
    }

    pub(super) fn end_optional_views_from(&mut self, mark: usize, span: crate::source::Span) {
        let guards: Vec<_> = self.active_optional_guards.drain(mark..).rev().collect();
        for guard in guards {
            self.emit(MirInstruction::EndOptionalView(
                crate::mir::MirOptionalViewEnd {
                    guard: guard.guard,
                    source: guard.source,
                    class: guard.class,
                    span,
                },
            ));
        }
    }
    pub(super) fn new_optional_storage(
        &mut self,
        kind: MirStorageKind,
        name: &str,
        ty: crate::hir::Type,
        span: crate::source::Span,
    ) -> StorageId {
        let mir_ty = match ty {
            Type::OptionalPrimitive(payload) => {
                MirType::OptionalPrimitive(lower_primitive_type(payload))
            }
            Type::OptionalClass(class) => MirType::OptionalClass(class),
            Type::OptionalShared(target) => {
                MirType::OptionalShared(super::lower_shared_target(target))
            }
            _ => unreachable!("optional storage requires an optional type"),
        };
        let id = StorageId::new(self.input.callable, self.storage.len());
        self.storage.push(MirStorage {
            id,
            source: None,
            name: format!("{name}-{}", id.index()),
            kind,
            ty: mir_ty,
            span,
        });
        id
    }
}

pub(super) const fn lower_primitive_type(
    payload: crate::hir::HirPrimitiveType,
) -> MirPrimitiveType {
    match payload {
        crate::hir::HirPrimitiveType::I64 => MirPrimitiveType::I64,
        crate::hir::HirPrimitiveType::U64 => MirPrimitiveType::U64,
        crate::hir::HirPrimitiveType::U8 => MirPrimitiveType::U8,
        crate::hir::HirPrimitiveType::F64 => MirPrimitiveType::F64,
        crate::hir::HirPrimitiveType::Bool => MirPrimitiveType::Bool,
    }
}
