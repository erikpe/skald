//! General iteration lowered entirely through ordinary MIR operations.

use super::*;
use crate::hir::{
    HirForIn, HirIterationValueInitialization, HirOptionalDestructionPlan,
    HirOptionalPresenceTestPlan, HirOptionalUnwrapPlan,
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
        let class_payload = matches!(
            statement.item.value.initialization,
            HirIterationValueInitialization::CopyClass { .. }
        );
        let guard_overflow =
            class_payload.then(|| self.body.allocate_block(statement.spans.binding_span));
        let payload_guard = class_payload.then(|| {
            let guard = OptionalGuardId::new(self.input.callable, self.next_optional_guard);
            self.next_optional_guard += 1;
            guard
        });

        let break_retained_depth = self.cleanup.retained_scope_depth();
        self.cleanup.enter_scope();

        // Core receivers are stable named places or forwarded interface
        // views. Acquire the view exactly once and retain it for both calls.
        let receiver = self.lower_object_view(&statement.receiver.view);
        debug_assert!(
            !self.full_expression.has_temporaries(),
            "core iteration receivers must not require call-duration temporaries"
        );

        let state = self.new_iteration_storage(
            "iteration-state",
            MirStorageKind::Local,
            self.lower_type(statement.protocol.state),
            statement.spans.iterable_span,
        );
        self.begin_storage_lifetime(state, statement.spans.iterable_span);
        self.cleanup.register_storage(state);
        let state_value = self.new_value(
            self.lower_type(statement.protocol.state),
            statement.spans.iterable_span,
        );
        self.emit(MirInstruction::Call(MirCall {
            target: MirCallTarget::Interface(MirInterfaceCallTarget {
                interface: statement.state.initialize.target.interface,
                requirement: statement.state.initialize.target.requirement,
            }),
            receiver: Some(receiver.clone().into()),
            arguments: Vec::new(),
            result: Some(state_value),
            shared_result: None,
            destination: None,
            span: statement.spans.iterable_span,
        }));
        self.emit(MirInstruction::Store(MirStore {
            destination: MirPlace::base(state),
            value: state_value,
            authorization: None,
            final_authorization: None,
            span: statement.spans.iterable_span,
        }));

        let result = self.new_iteration_storage(
            "iteration-result",
            MirStorageKind::Local,
            MirType::Optional(statement.result.optional),
            statement.spans.span,
        );
        let scalar_payload = matches!(
            statement.item.value.initialization,
            HirIterationValueInitialization::Trivial
        )
        .then(|| {
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
        self.emit(MirInstruction::Call(MirCall {
            target: MirCallTarget::Interface(MirInterfaceCallTarget {
                interface: statement.state.advance.target.interface,
                requirement: statement.state.advance.target.requirement,
            }),
            receiver: Some(receiver.clone().into()),
            arguments: vec![MirArgument::Place(MirPlace::base(state))],
            result: None,
            shared_result: None,
            destination: Some(MirPlace::base(result)),
            span: statement.spans.span,
        }));
        debug_assert_eq!(
            statement.result.presence,
            HirOptionalPresenceTestPlan::OuterTag
        );
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
        match statement.item.value.initialization {
            HirIterationValueInitialization::Trivial => {
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
            HirIterationValueInitialization::CopyClass { class, .. } => {
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
        self.begin_storage_lifetime(item, statement.spans.binding_span);
        self.cleanup.register_storage(item);
        match statement.item.value.initialization {
            HirIterationValueInitialization::Trivial => {
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
            HirIterationValueInitialization::CopyClass { class, operation } => {
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
                self.cleanup.register_owned(item, class);
            }
        }
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
            _ => unreachable!("the core iteration result matrix is primitive or inline class"),
        }
        self.end_storage_lifetime(result, statement.spans.span);
    }
}
