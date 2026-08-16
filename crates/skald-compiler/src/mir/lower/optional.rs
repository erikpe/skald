//! Primitive optional storage and checked-access lowering.

use crate::{
    hir::{
        HirClassOptionalAssignment, HirClassOptionalInitialize, HirClassOptionalPlace,
        HirClassOptionalSource, HirObjectSource, HirOptionalOperand, HirOptionalPlace,
        HirOptionalSource, HirOptionalStorage, HirPresenceTestKind, Type,
    },
    mir::{
        MirAggregateOptionalAssign, MirAggregateOptionalInitialize, MirAggregateOptionalPublish,
        MirAggregateOptionalSource, MirClassOptionalAssign, MirClassOptionalInitialize,
        MirClassOptionalPublish, MirClassOptionalSource, MirInstruction, MirOptionalAssign,
        MirOptionalInitialize, MirOptionalSource, MirPresenceTestKind, MirRvalueKind, MirStorage,
        MirStorageKind, MirTerminationReason, MirTerminator, MirType, StorageId,
    },
};

use super::{optional_types, BodyLowerer};

impl BodyLowerer<'_> {
    /// Lowers one already-selected HIR initialization directly into its final
    /// destination. Recursive optionals, statics, fields, and array element
    /// lists all share this path so lifecycle ordering cannot drift.
    pub(super) fn lower_stored_value_initialize_at(
        &mut self,
        destination: crate::mir::MirPlace,
        destination_type: Type,
        initialization: &crate::hir::HirStoredValueInitialization,
        span: crate::source::Span,
    ) {
        use crate::hir::{HirObjectDestinationInitialization, HirStoredValueInitialization};
        match initialization {
            HirStoredValueInitialization::Scalar(expression) => {
                let value = self
                    .lower_expression(expression)
                    .expect("typed primitive initialization must produce a MIR value");
                self.emit(MirInstruction::Store(crate::mir::MirStore {
                    destination,
                    value,
                    authorization: None,
                    final_authorization: None,
                    span,
                }));
            }
            HirStoredValueInitialization::Class(initialization) => {
                let Type::Class(class) = destination_type else {
                    unreachable!("class initialization requires exact-class storage")
                };
                match initialization {
                    HirObjectDestinationInitialization::Direct { producer, .. } => {
                        self.lower_object_producer(producer, destination);
                    }
                    HirObjectDestinationInitialization::Copy {
                        source, operation, ..
                    } => {
                        let optional_mark = self.optional_view_mark();
                        let source = self.lower_object_source(source);
                        self.emit(MirInstruction::CopyConstruct(
                            crate::mir::MirCopyConstruction {
                                destination,
                                source,
                                class,
                                operation: super::lower_selected_copy_operation(*operation),
                                span,
                            },
                        ));
                        self.end_optional_views_from(optional_mark, span);
                    }
                }
            }
            HirStoredValueInitialization::OptionalPrimitive { source, .. } => {
                self.lower_optional_initialize_at(destination, source, span);
            }
            HirStoredValueInitialization::OptionalClass(initialization) => {
                self.lower_class_optional_destination_initialize(destination, initialization);
            }
            HirStoredValueInitialization::Array(initialization) => {
                self.lower_array_initialize(destination, initialization, false);
            }
            HirStoredValueInitialization::Shared(transfer) => {
                let source = self.new_shared_temporary(transfer.target, transfer.span);
                self.lower_shared_transfer(source, transfer);
                self.consume_shared_temporary(source);
                self.emit(MirInstruction::SharedFieldInitialize(
                    crate::mir::MirSharedFieldInitialize {
                        destination,
                        source,
                        span,
                    },
                ));
            }
            HirStoredValueInitialization::OptionalShared(initialization) => {
                self.lower_optional_shared_initialize_at(destination, initialization);
            }
            HirStoredValueInitialization::Optional(value) => {
                self.lower_aggregate_optional_initialize_at(destination, value);
            }
            HirStoredValueInitialization::OptionalBoxPointeeCopy { .. } => unreachable!(
                "optional-box pointee copies require an optional-box allocation destination"
            ),
        }
    }

    pub(super) fn lower_aggregate_optional_initialize_at(
        &mut self,
        destination: crate::mir::MirPlace,
        value: &crate::hir::HirOptionalValue,
    ) {
        use crate::hir::HirOptionalValueSource;
        match &value.source {
            HirOptionalValueSource::Absent => {
                self.emit(MirInstruction::AggregateOptionalInitialize(
                    MirAggregateOptionalInitialize {
                        optional: value.optional,
                        destination,
                        source: MirAggregateOptionalSource::Absent,
                        span: value.span,
                    },
                ));
            }
            HirOptionalValueSource::Copy(source) => {
                let source = self.lower_aggregate_optional_place(source);
                self.emit(MirInstruction::AggregateOptionalInitialize(
                    MirAggregateOptionalInitialize {
                        optional: value.optional,
                        destination,
                        source: MirAggregateOptionalSource::Copy(source),
                        span: value.span,
                    },
                ));
            }
            HirOptionalValueSource::Produced(expression) => {
                self.lower_optional_call(expression, destination);
            }
            HirOptionalValueSource::Present(payload) => {
                self.emit(MirInstruction::AggregateOptionalInitialize(
                    MirAggregateOptionalInitialize {
                        optional: value.optional,
                        destination: destination.clone(),
                        source: MirAggregateOptionalSource::Unpublished,
                        span: value.span,
                    },
                ));
                let payload_type = self
                    .input
                    .optional_types
                    .get(value.optional)
                    .expect("typed recursive optional must have metadata")
                    .payload;
                self.lower_stored_value_initialize_at(
                    destination
                        .clone()
                        .project_aggregate_optional_payload(value.optional),
                    payload_type,
                    payload,
                    value.span,
                );
                self.emit(MirInstruction::AggregateOptionalPublish(
                    MirAggregateOptionalPublish {
                        optional: value.optional,
                        destination,
                        span: value.span,
                    },
                ));
            }
        }
    }

    pub(super) fn lower_aggregate_optional_assignment(
        &mut self,
        assignment: &crate::hir::HirAggregateOptionalAssignment,
    ) {
        let destination = self.lower_aggregate_optional_place(&assignment.destination);
        if assignment.kind == crate::hir::HirOptionalWriteKind::Initialize {
            self.lower_aggregate_optional_initialize_at(destination, &assignment.value);
            return;
        }
        let source = match &assignment.value.source {
            crate::hir::HirOptionalValueSource::Absent => MirAggregateOptionalSource::Absent,
            crate::hir::HirOptionalValueSource::Copy(source) => {
                MirAggregateOptionalSource::Copy(self.lower_aggregate_optional_place(source))
            }
            crate::hir::HirOptionalValueSource::Present(_)
            | crate::hir::HirOptionalValueSource::Produced(_) => {
                let temporary = self.new_optional_storage(
                    MirStorageKind::Temporary,
                    "aggregate-optional-source",
                    MirType::Optional(assignment.value.optional),
                    assignment.value.span,
                );
                let place = crate::mir::MirPlace::base(temporary);
                self.lower_aggregate_optional_initialize_at(place.clone(), &assignment.value);
                self.full_expression.register_temporary(
                    super::FullExpressionTemporary::AggregateOptional(
                        crate::mir::MirAggregateOptionalCleanup {
                            optional: assignment.value.optional,
                            destination: place.clone(),
                            span: assignment.value.span,
                        },
                    ),
                );
                MirAggregateOptionalSource::Copy(place)
            }
        };
        let self_copy =
            matches!(&source, MirAggregateOptionalSource::Copy(source) if source == &destination);
        if !self_copy {
            self.check_optional_mutation(destination.clone(), assignment.span);
        }
        self.emit(MirInstruction::AggregateOptionalAssign(
            MirAggregateOptionalAssign {
                optional: assignment.value.optional,
                destination,
                source,
                authorization: super::lower_optional_cell_write_authorization(
                    &assignment.destination.storage,
                ),
                final_authorization: super::lower_optional_final_write_authorization(
                    &assignment.destination.storage,
                ),
                span: assignment.span,
            },
        ));
    }

    pub(super) fn lower_aggregate_optional_place(
        &mut self,
        place: &crate::hir::HirOptionalValuePlace,
    ) -> crate::mir::MirPlace {
        self.lower_optional_storage(&place.storage)
    }

    pub(super) fn lower_nested_optional_unwrap_at(
        &mut self,
        destination: crate::mir::MirPlace,
        unwrap: &crate::hir::HirNestedOptionalUnwrap,
    ) {
        let source = self.lower_optional_operand(&unwrap.source);
        let present = self.assign(
            MirRvalueKind::OptionalPresence {
                source: source.clone(),
                kind: MirPresenceTestKind::Some,
            },
            MirType::Bool,
            unwrap.span,
        );
        let success_target = self.body.allocate_block(unwrap.span);
        let failure_target = self.body.allocate_block(unwrap.span);
        self.terminate(MirTerminator::Branch {
            condition: present,
            true_target: success_target,
            false_target: failure_target,
            span: unwrap.span,
        });
        self.body
            .select_block(failure_target)
            .expect("allocated nested-optional failure block must be selectable");
        self.terminate(MirTerminator::Terminate {
            reason: MirTerminationReason::OptionalAccessFailure,
            span: unwrap.span,
        });
        self.body
            .select_block(success_target)
            .expect("allocated nested-optional success block must be selectable");
        self.lower_optional_copy_initialize_at(
            destination,
            unwrap.payload,
            source.project_aggregate_optional_payload(unwrap.optional),
            unwrap.span,
        );
    }

    pub(super) fn lower_optional_array_unwrap(
        &mut self,
        destination: StorageId,
        unwrap: &crate::hir::HirOptionalArrayUnwrap,
    ) {
        let source = self.lower_optional_operand(&unwrap.source);
        let present = self.assign(
            MirRvalueKind::OptionalPresence {
                source: source.clone(),
                kind: MirPresenceTestKind::Some,
            },
            MirType::Bool,
            unwrap.span,
        );
        let success_target = self.body.allocate_block(unwrap.span);
        let failure_target = self.body.allocate_block(unwrap.span);
        self.terminate(MirTerminator::Branch {
            condition: present,
            true_target: success_target,
            false_target: failure_target,
            span: unwrap.span,
        });
        self.body
            .select_block(failure_target)
            .expect("allocated optional-array failure block must be selectable");
        self.terminate(MirTerminator::Terminate {
            reason: MirTerminationReason::OptionalAccessFailure,
            span: unwrap.span,
        });
        self.body
            .select_block(success_target)
            .expect("allocated optional-array success block must be selectable");
        let operation = self
            .input
            .array_types
            .get(unwrap.array)
            .and_then(|array| array.lifecycle.copy)
            .expect("typed optional-array unwrap must select array copying");
        let produced = self.lower_array_copy_from_place(
            unwrap.array,
            source.project_aggregate_optional_payload(unwrap.optional),
            super::lower_array_copy_element(operation),
            unwrap.span,
        );
        self.consume_array_temporary(produced);
        self.emit(MirInstruction::Array(
            crate::mir::MirArrayInstruction::Adopt {
                destination: crate::mir::MirPlace::base(destination),
                source: produced,
                array: unwrap.array,
                span: unwrap.span,
            },
        ));
    }

    pub(super) fn lower_optional_copy_initialize_at(
        &mut self,
        destination: crate::mir::MirPlace,
        optional: crate::identity::OptionalTypeId,
        source: crate::mir::MirPlace,
        span: crate::source::Span,
    ) {
        let metadata = self
            .input
            .optional_types
            .get(optional)
            .expect("typed optional copy must have metadata");
        match metadata.storage {
            crate::hir::HirOptionalStorageCategory::Scalar => {
                self.emit(MirInstruction::OptionalInitialize(MirOptionalInitialize {
                    destination,
                    source: MirOptionalSource::Copy(source),
                    span,
                }));
            }
            crate::hir::HirOptionalStorageCategory::InlineClass(class) => {
                let Some(crate::hir::HirOptionalCopyPlan::Class { operation, .. }) =
                    metadata.lifecycle.copy
                else {
                    unreachable!("copyable class optional must select a copy operation")
                };
                self.emit(MirInstruction::ClassOptionalInitialize(
                    MirClassOptionalInitialize {
                        optional,
                        destination,
                        source: MirClassOptionalSource::Copy(source),
                        class,
                        copy_constructor: Some(super::lower_selected_copy_operation(operation)),
                        span,
                    },
                ));
            }
            crate::hir::HirOptionalStorageCategory::SharedOwner(target) => {
                self.emit(MirInstruction::OptionalSharedInitialize(
                    crate::mir::MirOptionalSharedInitialize {
                        optional,
                        destination,
                        source: crate::mir::MirOptionalSharedSource::Copy(source),
                        target: super::lower_shared_target(target),
                        span,
                    },
                ));
            }
            crate::hir::HirOptionalStorageCategory::Nested(_)
            | crate::hir::HirOptionalStorageCategory::InlineArray(_) => {
                self.emit(MirInstruction::AggregateOptionalInitialize(
                    MirAggregateOptionalInitialize {
                        optional,
                        destination,
                        source: MirAggregateOptionalSource::Copy(source),
                        span,
                    },
                ));
            }
        }
    }

    fn register_optional_temporary(
        &mut self,
        storage: StorageId,
        optional: crate::identity::OptionalTypeId,
        span: crate::source::Span,
    ) {
        let metadata = self
            .input
            .optional_types
            .get(optional)
            .expect("typed optional temporary must have metadata");
        let destination = crate::mir::MirPlace::base(storage);
        let temporary = match metadata.storage {
            crate::hir::HirOptionalStorageCategory::Scalar => return,
            crate::hir::HirOptionalStorageCategory::InlineClass(class) => {
                super::FullExpressionTemporary::ClassOptional(crate::mir::MirClassOptionalCleanup {
                    optional,
                    destination,
                    class,
                    span,
                })
            }
            crate::hir::HirOptionalStorageCategory::SharedOwner(target) => {
                super::FullExpressionTemporary::OptionalShared(
                    crate::mir::MirOptionalSharedCleanup {
                        optional,
                        destination,
                        target: super::lower_shared_target(target),
                        span,
                    },
                )
            }
            crate::hir::HirOptionalStorageCategory::Nested(_)
            | crate::hir::HirOptionalStorageCategory::InlineArray(_) => {
                super::FullExpressionTemporary::AggregateOptional(
                    crate::mir::MirAggregateOptionalCleanup {
                        optional,
                        destination,
                        span,
                    },
                )
            }
        };
        self.full_expression.register_temporary(temporary);
    }

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
                        optional: optional_types::class_id(self.input.optional_types, value.class),
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
                        optional: optional_types::class_id(self.input.optional_types, value.class),
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
                        optional: optional_types::class_id(self.input.optional_types, value.class),
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

    pub(super) fn lower_class_optional_destination_initialize(
        &mut self,
        destination: crate::mir::MirPlace,
        value: &crate::hir::HirClassOptionalDestinationInitialization,
    ) {
        match value {
            crate::hir::HirClassOptionalDestinationInitialization::Absent { class, span } => {
                self.lower_class_optional_initialize_at(
                    destination,
                    &HirClassOptionalInitialize {
                        class: *class,
                        source: HirClassOptionalSource::Absent { span: *span },
                        copy_constructor: None,
                        span: *span,
                    },
                );
            }
            crate::hir::HirClassOptionalDestinationInitialization::Direct {
                class,
                producer,
                span,
            } => self.lower_class_optional_initialize_at(
                destination,
                &HirClassOptionalInitialize {
                    class: *class,
                    source: HirClassOptionalSource::Present(HirObjectSource::Produced(
                        producer.clone(),
                    )),
                    copy_constructor: None,
                    span: *span,
                },
            ),
            crate::hir::HirClassOptionalDestinationInitialization::Copy {
                class,
                source,
                operation,
                span,
            } => {
                let optional_mark = self.optional_view_mark();
                let source = self.lower_class_optional_copy_source(source, *class);
                self.emit(MirInstruction::ClassOptionalInitialize(
                    MirClassOptionalInitialize {
                        optional: optional_types::class_id(self.input.optional_types, *class),
                        destination,
                        source,
                        class: *class,
                        copy_constructor: Some(super::lower_selected_copy_operation(*operation)),
                        span: *span,
                    },
                ));
                self.end_optional_views_from(optional_mark, *span);
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
        let source =
            self.lower_class_optional_copy_source(&assignment.source, assignment.destination.class);
        let self_copy =
            matches!(&source, MirClassOptionalSource::Copy(source) if source == &destination);
        if !self_copy {
            self.check_optional_mutation(destination.clone(), assignment.span);
        }
        self.emit(MirInstruction::ClassOptionalAssign(
            MirClassOptionalAssign {
                optional: optional_types::class_id(
                    self.input.optional_types,
                    assignment.destination.class,
                ),
                destination,
                source,
                class: assignment.destination.class,
                copy_constructor: assignment
                    .copy_constructor
                    .map(super::lower_selected_copy_operation),
                copy_assignment: assignment
                    .copy_assignment
                    .map(super::lower_selected_copy_operation),
                authorization: super::lower_optional_cell_write_authorization(
                    &assignment.destination.storage,
                ),
                final_authorization: super::lower_optional_final_write_authorization(
                    &assignment.destination.storage,
                ),
                span: assignment.span,
            },
        ));
    }

    pub(super) fn lower_class_optional_copy_source(
        &mut self,
        source: &HirClassOptionalSource,
        class: crate::identity::ClassId,
    ) -> MirClassOptionalSource {
        match source {
            HirClassOptionalSource::Produced(expression) => {
                let storage = self.new_optional_storage(
                    MirStorageKind::Temporary,
                    "class-optional-result",
                    self.lower_type(expression.ty),
                    expression.span,
                );
                self.lower_optional_call(expression, crate::mir::MirPlace::base(storage));
                self.full_expression.register_temporary(
                    super::FullExpressionTemporary::ClassOptional(
                        crate::mir::MirClassOptionalCleanup {
                            optional: optional_types::class_id(self.input.optional_types, class),
                            destination: crate::mir::MirPlace::base(storage),
                            class,
                            span: expression.span,
                        },
                    ),
                );
                MirClassOptionalSource::Copy(crate::mir::MirPlace::base(storage))
            }
            source => self.lower_class_optional_source(source),
        }
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

    pub(super) fn emit_aggregate_optional_cleanup(
        &mut self,
        cleanup: crate::mir::MirAggregateOptionalCleanup,
    ) {
        self.check_optional_mutation(cleanup.destination.clone(), cleanup.span);
        self.emit(MirInstruction::AggregateOptionalCleanup(cleanup));
    }

    pub(super) fn lower_class_optional_source(
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
        self.lower_optional_storage(&place.storage)
    }

    pub(super) fn lower_optional_initialize(
        &mut self,
        destination: StorageId,
        source: &HirOptionalSource,
        span: crate::source::Span,
    ) {
        self.lower_optional_initialize_at(crate::mir::MirPlace::base(destination), source, span);
    }

    pub(super) fn lower_optional_initialize_at(
        &mut self,
        destination: crate::mir::MirPlace,
        source: &HirOptionalSource,
        span: crate::source::Span,
    ) {
        let source = self.lower_optional_source(source);
        self.emit(MirInstruction::OptionalInitialize(MirOptionalInitialize {
            destination,
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
                    authorization: super::lower_optional_cell_write_authorization(
                        &assignment.destination.storage,
                    ),
                    final_authorization: super::lower_optional_final_write_authorization(
                        &assignment.destination.storage,
                    ),
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

    pub(super) fn lower_optional_box_presence(
        &mut self,
        expression: &crate::hir::HirExpression,
        presence: &crate::hir::HirOptionalBoxPresence,
    ) -> crate::mir::ValueId {
        let owner = match &presence.source {
            crate::hir::HirSharedSource::Place(crate::hir::HirSharedPlace::Binding {
                binding,
                ..
            }) => self.storage_for_binding(*binding),
            source => self.new_shared_anchor(source, presence.span),
        };
        self.assign(
            MirRvalueKind::OptionalBoxPresence {
                owner,
                target: presence.box_target,
                layer: 0,
                kind: match presence.kind {
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
        let payload = super::primitive::lower_primitive_type(optional_types::primitive_payload(
            self.input.optional_types,
            source,
        ));
        let destination = StorageId::new(self.input.callable, self.storage.len());
        self.storage.push(MirStorage {
            id: destination,
            source: None,
            name: format!("unwrap{}", destination.index()),
            kind: MirStorageKind::OptionalUnwrap,
            ty: payload.payload_type(),
            span: expression.span,
        });
        self.track_full_expression_storage(destination, expression.span);

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
                    self.lower_type(expression.ty),
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
        self.lower_optional_storage(&place.storage)
    }

    pub(super) fn lower_optional_shared_initialize(
        &mut self,
        destination: StorageId,
        value: &crate::hir::HirOptionalSharedInitialize,
    ) {
        self.lower_optional_shared_initialize_at(crate::mir::MirPlace::base(destination), value);
    }

    pub(super) fn lower_optional_shared_initialize_at(
        &mut self,
        destination: crate::mir::MirPlace,
        value: &crate::hir::HirOptionalSharedInitialize,
    ) {
        let source = self.lower_optional_shared_source(&value.source);
        self.emit(MirInstruction::OptionalSharedInitialize(
            crate::mir::MirOptionalSharedInitialize {
                optional: optional_types::shared_id(self.input.optional_types, value.target),
                destination,
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
            optional: optional_types::shared_id(
                self.input.optional_types,
                assignment.destination.target,
            ),
            destination,
            source,
            target: super::lower_shared_target(assignment.destination.target),
            authorization: super::lower_optional_cell_write_authorization(
                &assignment.destination.storage,
            ),
            final_authorization: super::lower_optional_final_write_authorization(
                &assignment.destination.storage,
            ),
            span: assignment.span,
        };
        match assignment.kind {
            crate::hir::HirOptionalWriteKind::Initialize => {
                self.emit(MirInstruction::OptionalSharedInitialize(
                    crate::mir::MirOptionalSharedInitialize {
                        optional: optional_types::shared_id(
                            self.input.optional_types,
                            assignment.destination.target,
                        ),
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

    pub(super) fn lower_optional_shared_source(
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
                let Type::Optional(optional) = expression.ty else {
                    unreachable!("optional shared producer must have optional type")
                };
                let crate::hir::HirOptionalStorageCategory::SharedOwner(_) = self
                    .input
                    .optional_types
                    .get(optional)
                    .expect("typed optional identity must have metadata")
                    .storage
                else {
                    unreachable!("optional shared producer must have shared metadata")
                };
                let storage = self.new_optional_storage(
                    MirStorageKind::Temporary,
                    "optional-shared-result",
                    MirType::Optional(optional),
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
        self.lower_optional_storage(&place.storage)
    }

    fn lower_optional_storage(&mut self, storage: &HirOptionalStorage) -> crate::mir::MirPlace {
        match storage {
            HirOptionalStorage::Binding(binding) => self.lower_binding_place(*binding),
            HirOptionalStorage::Static(place) => crate::mir::MirPlace::static_field(place.field),
            HirOptionalStorage::Field(field) => self.lower_field_place(field),
            HirOptionalStorage::ArrayElement(element) => self.lower_array_element_place(element),
            HirOptionalStorage::SharedPointee(pointee) => {
                let owner = match &pointee.source {
                    crate::hir::HirSharedSource::Place(crate::hir::HirSharedPlace::Binding {
                        binding,
                        ..
                    }) => self.storage_for_binding(*binding),
                    source => self.new_shared_anchor(source, pointee.span),
                };
                crate::mir::MirPlace::shared_pointee(owner)
            }
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
        self.track_full_expression_storage(storage, span);
        storage
    }

    pub(super) fn lower_optional_shared_unwrap(
        &mut self,
        operand: &HirOptionalOperand,
        destination: StorageId,
    ) {
        let source = self.lower_optional_operand(operand);
        let target = super::lower_shared_target(optional_types::shared_payload(
            self.input.optional_types,
            operand,
        ));
        let success_target = self.body.allocate_block(operand.span());
        let failure_target = self.body.allocate_block(operand.span());
        self.terminate(MirTerminator::OptionalSharedUnwrap {
            unwrap: crate::mir::MirOptionalSharedUnwrap {
                optional: optional_types::shared_id(
                    self.input.optional_types,
                    optional_types::shared_payload(self.input.optional_types, operand),
                ),
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
                    self.lower_type(expression.ty),
                    expression.span,
                );
                self.lower_optional_call(expression, crate::mir::MirPlace::base(destination));
                let Type::Optional(optional) = expression.ty else {
                    unreachable!()
                };
                self.register_optional_temporary(destination, optional, expression.span);
                crate::mir::MirPlace::base(destination)
            }
            HirOptionalOperand::ClassPlace(place) => self.lower_class_optional_place(place),
            HirOptionalOperand::ClassProduced(expression) => {
                let destination = self.new_optional_storage(
                    MirStorageKind::Temporary,
                    "class-optional-result",
                    self.lower_type(expression.ty),
                    expression.span,
                );
                self.lower_optional_call(expression, crate::mir::MirPlace::base(destination));
                let Type::Optional(optional) = expression.ty else {
                    unreachable!()
                };
                self.register_optional_temporary(destination, optional, expression.span);
                crate::mir::MirPlace::base(destination)
            }
            HirOptionalOperand::SharedPlace(place) => self.lower_optional_shared_place(place),
            HirOptionalOperand::SharedProduced(expression) => {
                let destination = self.new_optional_storage(
                    MirStorageKind::Temporary,
                    "optional-shared-result",
                    self.lower_type(expression.ty),
                    expression.span,
                );
                self.lower_optional_shared_call(expression, destination);
                let Type::Optional(optional) = expression.ty else {
                    unreachable!()
                };
                self.register_optional_temporary(destination, optional, expression.span);
                crate::mir::MirPlace::base(destination)
            }
            HirOptionalOperand::AggregatePlace(place) => self.lower_aggregate_optional_place(place),
            HirOptionalOperand::AggregateProduced(expression) => {
                let Type::Optional(optional) = expression.ty else {
                    unreachable!()
                };
                let destination = self.new_optional_storage(
                    MirStorageKind::Temporary,
                    "aggregate-optional-result",
                    MirType::Optional(optional),
                    expression.span,
                );
                self.lower_optional_call(expression, crate::mir::MirPlace::base(destination));
                self.register_optional_temporary(destination, optional, expression.span);
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
        let class = optional_types::class_payload(self.input.optional_types, &view.source);
        let optional = optional_types::class_id(self.input.optional_types, class);
        self.begin_optional_payload_view(source, optional, MirType::Class(class), view.span)
            .project_optional_payload(class)
    }

    pub(super) fn begin_optional_payload_view(
        &mut self,
        source: crate::mir::MirPlace,
        optional: crate::identity::OptionalTypeId,
        payload: MirType,
        span: crate::source::Span,
    ) -> crate::mir::MirPlace {
        let guard = crate::mir::OptionalGuardId::new(self.input.callable, self.next_optional_guard);
        self.next_optional_guard += 1;
        let success_target = self.body.allocate_block(span);
        let absent_target = self.body.allocate_block(span);
        let overflow_target = self.body.allocate_block(span);
        self.terminate(MirTerminator::BeginOptionalView {
            begin: crate::mir::MirOptionalViewBegin {
                optional,
                guard,
                source: source.clone(),
                payload,
                span,
            },
            success_target,
            absent_target,
            overflow_target,
            span,
        });
        self.body
            .select_block(absent_target)
            .expect("allocated optional-view absence block must be selectable");
        self.terminate(MirTerminator::Terminate {
            reason: MirTerminationReason::OptionalAccessFailure,
            span,
        });
        self.body
            .select_block(overflow_target)
            .expect("allocated optional-view overflow block must be selectable");
        self.terminate(MirTerminator::Terminate {
            reason: MirTerminationReason::OptionalGuardOverflow,
            span,
        });
        self.body
            .select_block(success_target)
            .expect("allocated optional-view success block must be selectable");
        self.active_optional_guards
            .push(super::ActiveOptionalGuard::Inline {
                guard,
                source: source.clone(),
                optional,
                payload,
            });
        source
    }

    pub(super) fn begin_optional_box_view(
        &mut self,
        owner: StorageId,
        target: crate::identity::OptionalBoxTypeId,
        span: crate::source::Span,
    ) -> crate::mir::MirPlace {
        let metadata = self
            .input
            .optional_box_types
            .get(target)
            .expect("checked optional-box view must name metadata");
        for layer in 0..metadata.optional_depth {
            let guard =
                crate::mir::OptionalGuardId::new(self.input.callable, self.next_optional_guard);
            self.next_optional_guard += 1;
            let success_target = self.body.allocate_block(span);
            let absent_target = self.body.allocate_block(span);
            let overflow_target = self.body.allocate_block(span);
            self.terminate(MirTerminator::BeginOptionalBoxView {
                begin: crate::mir::MirOptionalBoxViewBegin {
                    box_target: target,
                    layer,
                    guard,
                    owner,
                    span,
                },
                success_target,
                absent_target,
                overflow_target,
                span,
            });
            self.body
                .select_block(absent_target)
                .expect("allocated optional-box absence block must be selectable");
            self.terminate(MirTerminator::Terminate {
                reason: MirTerminationReason::OptionalAccessFailure,
                span,
            });
            self.body
                .select_block(overflow_target)
                .expect("allocated optional-box overflow block must be selectable");
            self.terminate(MirTerminator::Terminate {
                reason: MirTerminationReason::OptionalGuardOverflow,
                span,
            });
            self.body
                .select_block(success_target)
                .expect("allocated optional-box success block must be selectable");
            self.active_optional_guards
                .push(super::ActiveOptionalGuard::Box {
                    guard,
                    owner,
                    target,
                    layer,
                });
        }
        crate::mir::MirPlace::optional_box_payload(owner, target)
    }

    pub(super) fn end_optional_views_from(&mut self, mark: usize, span: crate::source::Span) {
        let guards: Vec<_> = self.active_optional_guards.drain(mark..).rev().collect();
        for guard in guards {
            self.emit(match guard {
                super::ActiveOptionalGuard::Inline {
                    guard,
                    source,
                    optional,
                    payload,
                } => MirInstruction::EndOptionalView(crate::mir::MirOptionalViewEnd {
                    optional,
                    guard,
                    source,
                    payload,
                    span,
                }),
                super::ActiveOptionalGuard::Box {
                    guard,
                    owner,
                    target,
                    layer,
                } => MirInstruction::EndOptionalBoxView(crate::mir::MirOptionalBoxViewEnd {
                    box_target: target,
                    layer,
                    guard,
                    owner,
                    span,
                }),
            });
        }
    }
    pub(super) fn new_optional_storage(
        &mut self,
        kind: MirStorageKind,
        name: &str,
        mir_ty: MirType,
        span: crate::source::Span,
    ) -> StorageId {
        debug_assert!(matches!(mir_ty, MirType::Optional(_)));
        let id = StorageId::new(self.input.callable, self.storage.len());
        self.storage.push(MirStorage {
            id,
            source: None,
            name: format!("{name}-{}", id.index()),
            kind,
            ty: mir_ty,
            span,
        });
        self.track_full_expression_storage(id, span);
        id
    }
}
