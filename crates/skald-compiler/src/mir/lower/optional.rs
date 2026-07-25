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

use super::BodyLowerer;

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

    fn lower_class_optional_place(
        &mut self,
        place: &HirClassOptionalPlace,
    ) -> crate::mir::MirPlace {
        match &place.storage {
            HirOptionalStorage::Binding(binding) => {
                crate::mir::MirPlace::base(self.storage_for_binding(*binding))
            }
            HirOptionalStorage::Field(field) => self.lower_field_place(field),
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
            HirOptionalStorage::Binding(binding) => {
                crate::mir::MirPlace::base(self.storage_for_binding(*binding))
            }
            HirOptionalStorage::Field(field) => self.lower_field_place(field),
        }
    }

    fn lower_optional_operand(&mut self, operand: &HirOptionalOperand) -> crate::mir::MirPlace {
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
