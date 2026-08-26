//! General iteration lowered entirely through ordinary MIR operations.

use super::*;
use crate::hir::{
    HirForIn, HirIterationReceiverCarrier, HirIterationStoredValuePlan, HirIterationValueCopy,
    HirIterationValueDestruction, HirOptionalDestructionPlan, HirOptionalPresenceTestPlan,
    HirOptionalUnwrapPlan, Type,
};

impl BodyLowerer<'_> {
    pub(super) fn lower_for_in(&mut self, statement: &HirForIn) {
        let reaches_latch = statement.body.effects.can_fall_through()
            || statement.body.effects.can_continue_to(statement.loop_id);

        // Allocate the complete source-level loop shape before emitting an
        // edge. Impossible-access failure blocks are allocated here too, so
        // their identities remain deterministic.
        let header = self.body.allocate_block(statement.spans.iterable_span);
        let present = self.body.allocate_block(statement.spans.binding_span);
        let body = self.body.allocate_block(statement.body.span);
        let latch = reaches_latch.then(|| self.body.allocate_block(statement.spans.span));
        let outer_cleanup = self.body.allocate_block(statement.spans.span);
        let exit = self.body.allocate_block(statement.spans.span);
        let access_failure = self.body.allocate_block(statement.spans.binding_span);
        let item_copy = statement
            .item
            .value
            .copy
            .expect("typed iteration item must be independently copyable");
        let class_payload = matches!(item_copy, HirIterationValueCopy::Class { .. });
        let guard_overflow =
            class_payload.then(|| self.body.allocate_block(statement.spans.binding_span));
        let payload_guard = class_payload.then(|| {
            let guard = OptionalGuardId::new(self.input.callable, self.next_optional_guard);
            self.next_optional_guard += 1;
            guard
        });

        let break_retained_depth = self.cleanup.retained_scope_depth();
        self.cleanup.enter_scope();

        // Acquire one read-only view and promote every supporting carrier from
        // full-expression duration to this loop's outer lexical scope.
        let optional_mark = self.optional_view_mark();
        let mut receiver = match &statement.receiver.carrier {
            HirIterationReceiverCarrier::View(view) => self.lower_iteration_object_view(view),
            HirIterationReceiverCarrier::Checked(view) => {
                self.lower_iteration_checked_object_view(view)
            }
        };
        self.promote_iteration_receiver_resources(optional_mark, statement.spans.iterable_span);
        receiver.provenance = MirViewProvenance::Ordinary;

        let state = self.new_iteration_storage(
            "iteration-state",
            MirStorageKind::Local,
            self.lower_type(statement.protocol.state),
            statement.spans.iterable_span,
        );
        self.begin_storage_lifetime(state, statement.spans.iterable_span);
        self.cleanup.register_storage(state);
        self.emit_iteration_state_call(statement, receiver.clone(), state);
        self.register_iteration_value(state, &statement.state.value);
        let state_array_anchor = match statement.protocol.state {
            Type::Array(array) => Some((
                self.new_iteration_storage(
                    "iteration-state-array-anchor",
                    MirStorageKind::ArrayAnchor(MirArrayAnchorKind::InlineBacking),
                    MirType::Array(array),
                    statement.spans.span,
                ),
                array,
            )),
            _ => None,
        };

        let result = self.new_iteration_storage(
            "iteration-result",
            MirStorageKind::Local,
            MirType::Optional(statement.result.optional),
            statement.spans.span,
        );
        let scalar_payload = matches!(item_copy, HirIterationValueCopy::Trivial).then(|| {
            self.new_iteration_storage(
                "iteration-payload",
                MirStorageKind::OptionalUnwrap,
                self.lower_type(statement.protocol.item),
                statement.spans.binding_span,
            )
        });

        self.terminate(MirTerminator::Goto {
            target: header,
            span: statement.spans.span,
        });

        self.body
            .select_block(header)
            .expect("allocated iteration header must be selectable");
        self.begin_storage_lifetime(result, statement.spans.span);
        if let Some((anchor, array)) = state_array_anchor {
            self.begin_storage_lifetime(anchor, statement.spans.span);
            self.emit(MirInstruction::Array(MirArrayInstruction::AnchorBegin {
                anchor,
                owner: MirPlace::base(state),
                array,
                kind: MirArrayAnchorKind::InlineBacking,
                span: statement.spans.span,
            }));
        }
        let optional_shared_result = matches!(
            statement.result.destruction,
            HirOptionalDestructionPlan::Shared(_)
        );
        self.emit(MirInstruction::Call(MirCall {
            target: MirCallTarget::Interface(MirInterfaceCallTarget {
                interface: statement.state.advance.target.interface,
                requirement: statement.state.advance.target.requirement,
            }),
            receiver: Some(receiver.clone().into()),
            arguments: vec![MirArgument::Place(MirPlace::base(state))],
            result: None,
            shared_result: optional_shared_result.then_some(result),
            destination: (!optional_shared_result).then(|| MirPlace::base(result)),
            span: statement.spans.span,
        }));
        if let Some((anchor, _)) = state_array_anchor {
            self.emit(MirInstruction::Array(MirArrayInstruction::AnchorEnd {
                anchor,
                span: statement.spans.span,
            }));
            self.end_storage_lifetime(anchor, statement.spans.span);
        }
        debug_assert!(matches!(
            statement.result.presence,
            HirOptionalPresenceTestPlan::OuterTag | HirOptionalPresenceTestPlan::SharedOwnerNull
        ));
        let is_present = self.assign(
            MirRvalueKind::OptionalPresence {
                source: MirPlace::base(result),
                kind: MirPresenceTestKind::Some,
            },
            MirType::Bool,
            statement.spans.span,
        );
        self.terminate(MirTerminator::Branch {
            condition: is_present,
            true_target: present,
            false_target: outer_cleanup,
            span: statement.spans.span,
        });

        self.body
            .select_block(present)
            .expect("allocated iteration presence block must be selectable");
        match item_copy {
            HirIterationValueCopy::Trivial => {
                debug_assert_eq!(
                    statement.result.unwrap,
                    HirOptionalUnwrapPlan::ExtractScalar
                );
                let payload = scalar_payload.expect("scalar iteration item needs payload storage");
                self.begin_storage_lifetime(payload, statement.spans.binding_span);
                self.terminate(MirTerminator::OptionalUnwrap {
                    source: MirPlace::base(result),
                    destination: payload,
                    success_target: body,
                    failure_target: access_failure,
                    span: statement.spans.binding_span,
                });
            }
            HirIterationValueCopy::Class { class, .. } => {
                debug_assert_eq!(
                    statement.result.unwrap,
                    HirOptionalUnwrapPlan::CheckedInlineClass(class)
                );
                self.terminate(MirTerminator::BeginOptionalView {
                    begin: MirOptionalViewBegin {
                        optional: statement.result.optional,
                        guard: payload_guard.expect("class iteration item needs a payload guard"),
                        source: MirPlace::base(result),
                        payload: MirType::Class(class),
                        span: statement.spans.binding_span,
                    },
                    success_target: body,
                    absent_target: access_failure,
                    overflow_target: guard_overflow
                        .expect("class iteration item needs a guard-overflow block"),
                    span: statement.spans.binding_span,
                });
            }
            HirIterationValueCopy::Shared(target) => {
                debug_assert_eq!(
                    statement.result.unwrap,
                    HirOptionalUnwrapPlan::SecureSharedOwner(target)
                );
                let item = self.local_storage[statement.binding.index()];
                self.begin_storage_lifetime(item, statement.spans.binding_span);
                self.terminate(MirTerminator::OptionalSharedUnwrap {
                    unwrap: MirOptionalSharedUnwrap {
                        optional: statement.result.optional,
                        source: MirPlace::base(result),
                        destination: item,
                        target: lower_shared_target(target),
                        span: statement.spans.binding_span,
                    },
                    success_target: body,
                    failure_target: access_failure,
                    span: statement.spans.binding_span,
                });
            }
            HirIterationValueCopy::Array { array, .. } => {
                debug_assert_eq!(
                    statement.result.unwrap,
                    HirOptionalUnwrapPlan::CheckedInlineArray(array)
                );
                self.terminate(MirTerminator::Goto {
                    target: body,
                    span: statement.spans.binding_span,
                });
            }
            HirIterationValueCopy::Optional { optional, .. } => {
                debug_assert_eq!(
                    statement.result.unwrap,
                    HirOptionalUnwrapPlan::CheckedNested(optional)
                );
                self.terminate(MirTerminator::Goto {
                    target: body,
                    span: statement.spans.binding_span,
                });
            }
        }

        self.body
            .select_block(access_failure)
            .expect("allocated optional-access failure block must be selectable");
        self.terminate(MirTerminator::Terminate {
            reason: MirTerminationReason::OptionalAccessFailure,
            span: statement.spans.binding_span,
        });
        if let Some(guard_overflow) = guard_overflow {
            self.body
                .select_block(guard_overflow)
                .expect("allocated optional-guard failure block must be selectable");
            self.terminate(MirTerminator::Terminate {
                reason: MirTerminationReason::OptionalGuardOverflow,
                span: statement.spans.binding_span,
            });
        }

        let continue_retained_depth = self.cleanup.retained_scope_depth();
        let context = loop_context::LoopContext::new(
            statement.loop_id,
            exit,
            latch,
            break_retained_depth,
            continue_retained_depth,
        )
        .expect("typed iteration and its MIR targets must share a callable owner");
        self.loop_contexts.push(context);

        self.body
            .select_block(body)
            .expect("allocated iteration body block must be selectable");
        self.cleanup.enter_scope();
        let item = self.local_storage[statement.binding.index()];
        if !matches!(item_copy, HirIterationValueCopy::Shared(_)) {
            self.begin_storage_lifetime(item, statement.spans.binding_span);
        }
        self.cleanup.register_storage(item);
        match item_copy {
            HirIterationValueCopy::Trivial => {
                let payload = scalar_payload.expect("scalar iteration item needs payload storage");
                let value = self.assign(
                    MirRvalueKind::Load(MirPlace::base(payload)),
                    self.lower_type(statement.protocol.item),
                    statement.spans.binding_span,
                );
                self.end_storage_lifetime(payload, statement.spans.binding_span);
                self.emit(MirInstruction::Store(MirStore {
                    destination: MirPlace::base(item),
                    value,
                    authorization: None,
                    final_authorization: None,
                    span: statement.spans.binding_span,
                }));
            }
            HirIterationValueCopy::Class { class, operation } => {
                self.active_optional_guards
                    .push(ActiveOptionalGuard::Inline {
                        guard: payload_guard.expect("class iteration item needs a payload guard"),
                        source: MirPlace::base(result),
                        optional: statement.result.optional,
                        payload: MirType::Class(class),
                    });
                self.emit(MirInstruction::CopyConstruct(MirCopyConstruction {
                    destination: MirPlace::base(item),
                    source: MirPlace::base(result).project_optional_payload(class),
                    class,
                    operation: lower_selected_copy_operation(operation),
                    span: statement.spans.binding_span,
                }));
                self.end_optional_views_from(0, statement.spans.binding_span);
            }
            HirIterationValueCopy::Array { array, operation } => {
                let produced = self.lower_array_copy_from_place(
                    array,
                    MirPlace::base(result)
                        .project_aggregate_optional_payload(statement.result.optional),
                    lower_array_copy_element(operation),
                    statement.spans.binding_span,
                );
                self.consume_array_temporary(produced);
                self.emit(MirInstruction::Array(MirArrayInstruction::Adopt {
                    destination: MirPlace::base(item),
                    source: produced,
                    array,
                    span: statement.spans.binding_span,
                }));
                self.finish_full_expression(statement.spans.binding_span);
            }
            HirIterationValueCopy::Shared(_) => {
                // The optional-shared unwrap edge initialized the item and
                // retained its independent strong owner before body entry.
            }
            HirIterationValueCopy::Optional {
                optional,
                operation,
            } => {
                let metadata = self
                    .input
                    .optional_types
                    .get(optional)
                    .expect("typed iteration optional item must have metadata");
                debug_assert_eq!(metadata.lifecycle.copy, Some(operation));
                self.lower_optional_copy_initialize_at(
                    MirPlace::base(item),
                    optional,
                    MirPlace::base(result)
                        .project_aggregate_optional_payload(statement.result.optional),
                    statement.spans.binding_span,
                );
            }
        }
        self.register_iteration_value(item, &statement.item.value);
        self.cleanup_iteration_result(statement, result);
        self.lower_block(&statement.body);
        if !self.body.is_current_terminated() {
            self.emit_scope_exit(self.cleanup.for_current_scope(statement.spans.span));
            self.terminate(MirTerminator::Goto {
                target: latch.expect("a falling-through iteration body requires a latch"),
                span: statement.spans.span,
            });
        }
        self.cleanup.leave_scope();
        self.loop_contexts.pop(statement.loop_id);

        if let Some(latch) = latch {
            self.body
                .select_block(latch)
                .expect("allocated iteration latch must be selectable");
            self.terminate(MirTerminator::Goto {
                target: header,
                span: statement.spans.span,
            });
        }

        self.body
            .select_block(outer_cleanup)
            .expect("allocated iteration outer-cleanup block must be selectable");
        self.cleanup_iteration_result(statement, result);
        self.emit_scope_exit(self.cleanup.for_current_scope(statement.spans.span));
        self.terminate(MirTerminator::Goto {
            target: exit,
            span: statement.spans.span,
        });
        self.cleanup.leave_scope();

        self.body
            .select_block(exit)
            .expect("allocated iteration exit must be selectable");
    }

    fn new_iteration_storage(
        &mut self,
        name: &str,
        kind: MirStorageKind,
        ty: MirType,
        span: crate::source::Span,
    ) -> StorageId {
        let id = StorageId::new(self.input.callable, self.storage.len());
        self.storage.push(MirStorage {
            id,
            source: None,
            name: format!("{name}-{}", id.index()),
            kind,
            ty,
            span,
        });
        id
    }

    fn emit_iteration_state_call(
        &mut self,
        statement: &HirForIn,
        receiver: MirObjectView,
        state: StorageId,
    ) {
        let ty = statement.state.value.ty;
        let scalar = matches!(
            ty,
            Type::I64 | Type::U64 | Type::U8 | Type::F64 | Type::Bool
        );
        let shared = matches!(ty, Type::Shared(_))
            || matches!(
                statement.state.value.destruction,
                HirIterationValueDestruction::Optional {
                    plan: HirOptionalDestructionPlan::Shared(_),
                    ..
                }
            );
        let result = scalar.then(|| {
            self.new_value(
                self.lower_type(statement.protocol.state),
                statement.spans.iterable_span,
            )
        });
        self.emit(MirInstruction::Call(MirCall {
            target: MirCallTarget::Interface(MirInterfaceCallTarget {
                interface: statement.state.initialize.target.interface,
                requirement: statement.state.initialize.target.requirement,
            }),
            receiver: Some(receiver.into()),
            arguments: Vec::new(),
            result,
            shared_result: shared.then_some(state),
            destination: (!scalar && !shared).then(|| MirPlace::base(state)),
            span: statement.spans.iterable_span,
        }));
        if let Some(value) = result {
            self.emit(MirInstruction::Store(MirStore {
                destination: MirPlace::base(state),
                value,
                authorization: None,
                final_authorization: None,
                span: statement.spans.iterable_span,
            }));
        }
    }

    fn register_iteration_value(
        &mut self,
        storage: StorageId,
        value: &HirIterationStoredValuePlan,
    ) {
        match value.destruction {
            HirIterationValueDestruction::Trivial => {}
            HirIterationValueDestruction::Class(class) => {
                self.cleanup.register_owned(storage, class)
            }
            HirIterationValueDestruction::Array(array) => {
                self.cleanup.register_array(storage, array)
            }
            HirIterationValueDestruction::Shared(_) => self.cleanup.register_shared(storage),
            HirIterationValueDestruction::Optional { optional, plan } => match plan {
                HirOptionalDestructionPlan::Trivial => {}
                HirOptionalDestructionPlan::Class(class) => self
                    .cleanup
                    .register_class_optional(storage, optional, class),
                HirOptionalDestructionPlan::Shared(target) => self
                    .cleanup
                    .register_optional_shared(storage, optional, lower_shared_target(target)),
                HirOptionalDestructionPlan::Array(_) | HirOptionalDestructionPlan::Optional(_) => {
                    self.cleanup.register_aggregate_optional(storage, optional)
                }
            },
        }
    }

    fn cleanup_iteration_result(&mut self, statement: &HirForIn, result: StorageId) {
        match statement.result.destruction {
            HirOptionalDestructionPlan::Trivial => {}
            HirOptionalDestructionPlan::Class(class) => {
                self.emit_class_optional_cleanup(MirClassOptionalCleanup {
                    optional: statement.result.optional,
                    destination: MirPlace::base(result),
                    class,
                    span: statement.spans.span,
                });
            }
            HirOptionalDestructionPlan::Array(_) | HirOptionalDestructionPlan::Optional(_) => {
                self.emit_aggregate_optional_cleanup(MirAggregateOptionalCleanup {
                    optional: statement.result.optional,
                    destination: MirPlace::base(result),
                    span: statement.spans.span,
                });
            }
            HirOptionalDestructionPlan::Shared(target) => {
                self.emit(MirInstruction::OptionalSharedCleanup(
                    MirOptionalSharedCleanup {
                        optional: statement.result.optional,
                        destination: MirPlace::base(result),
                        target: lower_shared_target(target),
                        span: statement.spans.span,
                    },
                ));
                // The shared-result call and a successful shared unwrap both
                // establish ownership that must cross exactly one ordinary
                // full-expression boundary. Put that boundary after result
                // cleanup so it exists on both the yielded and terminating
                // attempt paths.
                self.emit(MirInstruction::EndFullExpression(MirEndFullExpression {
                    temporaries: Vec::new(),
                    span: statement.spans.span,
                }));
            }
        }
        self.end_storage_lifetime(result, statement.spans.span);
    }
}
