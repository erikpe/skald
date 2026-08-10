//! Lexical ownership state and cleanup planning for MIR control-flow edges.

use crate::{identity::ClassId, source::Span};

use super::{
    full_expression::{
        build_conditional_region, finish_conditional_region, ConditionalRegistration,
    },
    BodyLowerer, MirArrayInstruction, MirCheckedViewEnd, MirCleanup, MirEndFullExpression,
    MirInstruction, MirPathCondition, MirPlace, MirSharedRelease, StorageId,
};

impl BodyLowerer<'_> {
    /// Finish a boundary without carrying a block-local scalar across the
    /// conditional cleanup graph.
    pub(super) fn finish_full_expression_with_scalar(
        &mut self,
        value: super::ValueId,
        ty: super::MirType,
        span: Span,
    ) -> super::ValueId {
        if !self.full_expression.has_conditions() {
            self.finish_full_expression(span);
            return value;
        }

        let (storage, ty) = self.spill_scalar(value, ty, span);
        self.extend_storage_beyond_full_expression(storage);
        self.finish_full_expression(span);
        let value = self.assign(
            super::MirRvalueKind::Load(super::MirPlace::base(storage)),
            ty,
            span,
        );
        self.end_storage_lifetime(storage, span);
        value
    }

    pub(super) fn finish_full_expression(&mut self, span: Span) {
        self.end_optional_views_from(0, span);
        let plan = self.full_expression.take_plan();
        let requires_boundary = plan.requires_boundary();
        let conditions = plan.conditions.clone();

        for registration in plan.checked_views.into_iter().rev() {
            self.emit_conditional_registration(
                registration,
                &conditions,
                span,
                |lowerer, carrier| {
                    lowerer.emit(MirInstruction::EndCheckedView(MirCheckedViewEnd {
                        carrier,
                        span,
                    }));
                },
            );
        }
        if !requires_boundary {
            return;
        }

        self.emit_full_expression_temporaries(plan.temporaries, &conditions, span);
        for registration in plan.storage.into_iter().rev() {
            self.emit_conditional_registration(
                registration,
                &conditions,
                span,
                |lowerer, storage| lowerer.end_storage_lifetime(storage, span),
            );
        }
        self.end_path_condition_lifetimes(&conditions, span);
    }

    fn emit_full_expression_temporaries(
        &mut self,
        temporaries: Vec<ConditionalRegistration<super::FullExpressionTemporary>>,
        conditions: &[MirPathCondition],
        span: Span,
    ) {
        let mut inline = Vec::new();
        for registration in temporaries.into_iter().rev() {
            match (registration.condition, registration.value) {
                (None, super::FullExpressionTemporary::Inline(mut cleanup)) => {
                    cleanup.span = span;
                    inline.push(cleanup);
                }
                (condition, temporary) => {
                    self.flush_inline_temporaries(&mut inline, span);
                    let registration = ConditionalRegistration {
                        condition,
                        value: temporary,
                    };
                    self.emit_conditional_registration(
                        registration,
                        conditions,
                        span,
                        |lowerer, temporary| {
                            lowerer.emit_full_expression_temporary(temporary, span);
                        },
                    );
                }
            }
        }
        // Keep one explicit boundary even when all cleanup actions were
        // conditional or non-inline. Existing verifier domains use this as
        // the point after which expression temporaries must be exhausted.
        self.emit(MirInstruction::EndFullExpression(MirEndFullExpression {
            temporaries: inline,
            span,
        }));
    }

    fn flush_inline_temporaries(&mut self, inline: &mut Vec<MirCleanup>, span: Span) {
        if inline.is_empty() {
            return;
        }
        self.emit(MirInstruction::EndFullExpression(MirEndFullExpression {
            temporaries: std::mem::take(inline),
            span,
        }));
    }

    fn emit_full_expression_temporary(
        &mut self,
        temporary: super::FullExpressionTemporary,
        span: Span,
    ) {
        match temporary {
            super::FullExpressionTemporary::Inline(mut cleanup) => {
                cleanup.span = span;
                self.emit(MirInstruction::EndFullExpression(MirEndFullExpression {
                    temporaries: vec![cleanup],
                    span,
                }));
            }
            super::FullExpressionTemporary::Shared(owner) => {
                self.emit(MirInstruction::SharedRelease(MirSharedRelease {
                    owner,
                    span,
                }));
            }
            super::FullExpressionTemporary::ClassOptional(cleanup) => {
                self.emit_class_optional_cleanup(cleanup);
            }
            super::FullExpressionTemporary::OptionalShared(cleanup) => {
                self.emit(MirInstruction::OptionalSharedCleanup(cleanup));
            }
            super::FullExpressionTemporary::AggregateOptional(cleanup) => {
                self.emit_aggregate_optional_cleanup(cleanup);
            }
            super::FullExpressionTemporary::Array { storage, array } => {
                self.emit(MirInstruction::Array(MirArrayInstruction::Release {
                    owner: MirPlace::base(storage),
                    array,
                    span,
                }));
            }
            super::FullExpressionTemporary::ArrayAnchor(anchor) => {
                self.emit(MirInstruction::Array(MirArrayInstruction::AnchorEnd {
                    anchor,
                    span,
                }));
            }
        }
    }

    fn emit_conditional_registration<T>(
        &mut self,
        registration: ConditionalRegistration<T>,
        conditions: &[MirPathCondition],
        span: Span,
        emit: impl FnOnce(&mut Self, T),
    ) {
        let Some(condition) = registration.condition else {
            emit(self, registration.value);
            return;
        };

        let region = build_conditional_region(
            &mut self.body,
            &mut self.values,
            self.input.callable,
            condition,
            conditions,
            span,
        );
        emit(self, registration.value);
        finish_conditional_region(&mut self.body, region, span);
    }

    fn end_path_condition_lifetimes(&mut self, conditions: &[MirPathCondition], span: Span) {
        for condition in conditions.iter().rev() {
            let registration = ConditionalRegistration {
                condition: condition.parent,
                value: condition.activation,
            };
            self.emit_conditional_registration(
                registration,
                conditions,
                span,
                |lowerer, activation| lowerer.end_storage_lifetime(activation, span),
            );
        }
    }

    /// Drop path-local lowering bookkeeping after a non-unwinding terminator.
    ///
    /// The represented values intentionally remain live for the terminator;
    /// only the lowerer's traversal state is reset before it visits another
    /// independently selected basic block.
    pub(super) fn discard_terminated_full_expression_tracking(&mut self) {
        self.full_expression.clear();
        self.active_optional_guards.clear();
    }
}

/// Owning storage whose initialization completed on the current path.
#[derive(Clone, Copy)]
struct InitializedStorage {
    storage: StorageId,
    kind: OwnedStorageKind,
}

#[derive(Clone, Copy)]
enum OwnedStorageKind {
    Inline(ClassId),
    Shared,
    ClassOptional(crate::identity::OptionalTypeId, ClassId),
    OptionalShared(crate::identity::OptionalTypeId, crate::mir::MirSharedTarget),
    AggregateOptional(crate::identity::OptionalTypeId),
    Array(crate::identity::ArrayTypeId),
}

pub(super) enum PlannedCleanup {
    Inline(MirCleanup),
    Shared(MirSharedRelease),
    ClassOptional(crate::mir::MirClassOptionalCleanup),
    OptionalShared(crate::mir::MirOptionalSharedCleanup),
    AggregateOptional(crate::mir::MirAggregateOptionalCleanup),
    Array {
        storage: StorageId,
        array: crate::identity::ArrayTypeId,
        span: Span,
    },
}

pub(super) struct PlannedScopeExit {
    pub(super) cleanups: Vec<PlannedCleanup>,
    pub(super) storage: Vec<StorageId>,
    pub(super) span: Span,
}

impl PlannedScopeExit {
    pub(super) fn requires_optional_check(&self) -> bool {
        self.cleanups
            .iter()
            .any(PlannedCleanup::requires_optional_check)
    }
}

impl PlannedCleanup {
    pub(super) const fn requires_optional_check(&self) -> bool {
        matches!(self, Self::ClassOptional(_) | Self::AggregateOptional(_))
    }
}

/// Tracks lexical ownership independently from expression lowering.
///
/// Planning does not consume state: one source scope may have several outgoing
/// CFG edges, and each edge needs the same cleanup sequence. Leaving the source
/// scope is the only operation that discards its registrations.
pub(super) struct CleanupPlanner {
    scopes: Vec<LexicalScope>,
}

/// Opaque count of lexical scopes retained by a targeted control-flow edge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RetainedScopeDepth(usize);

#[derive(Default)]
struct LexicalScope {
    initialized: Vec<InitializedStorage>,
    storage: Vec<StorageId>,
}

impl CleanupPlanner {
    pub(super) const fn new() -> Self {
        Self { scopes: Vec::new() }
    }

    pub(super) fn enter_scope(&mut self) {
        self.scopes.push(LexicalScope::default());
    }

    #[allow(dead_code)]
    pub(super) fn retained_scope_depth(&self) -> RetainedScopeDepth {
        RetainedScopeDepth(self.scopes.len())
    }

    pub(super) fn register_storage(&mut self, storage: StorageId) {
        self.scopes
            .last_mut()
            .expect("live local storage must belong to an active lexical scope")
            .storage
            .push(storage);
    }

    pub(super) fn register_owned(&mut self, storage: StorageId, class: ClassId) {
        self.scopes
            .last_mut()
            .expect("an initialized local must belong to an active lexical scope")
            .initialized
            .push(InitializedStorage {
                storage,
                kind: OwnedStorageKind::Inline(class),
            });
    }

    pub(super) fn register_shared(&mut self, storage: StorageId) {
        self.scopes
            .last_mut()
            .expect("an initialized local must belong to an active lexical scope")
            .initialized
            .push(InitializedStorage {
                storage,
                kind: OwnedStorageKind::Shared,
            });
    }

    pub(super) fn register_class_optional(
        &mut self,
        storage: StorageId,
        optional: crate::identity::OptionalTypeId,
        class: ClassId,
    ) {
        self.scopes
            .last_mut()
            .expect("an initialized local must belong to an active lexical scope")
            .initialized
            .push(InitializedStorage {
                storage,
                kind: OwnedStorageKind::ClassOptional(optional, class),
            });
    }

    pub(super) fn register_optional_shared(
        &mut self,
        storage: StorageId,
        optional: crate::identity::OptionalTypeId,
        target: crate::mir::MirSharedTarget,
    ) {
        self.scopes
            .last_mut()
            .expect("an initialized local must belong to an active lexical scope")
            .initialized
            .push(InitializedStorage {
                storage,
                kind: OwnedStorageKind::OptionalShared(optional, target),
            });
    }

    pub(super) fn register_aggregate_optional(
        &mut self,
        storage: StorageId,
        optional: crate::identity::OptionalTypeId,
    ) {
        self.scopes
            .last_mut()
            .expect("an initialized local must belong to an active lexical scope")
            .initialized
            .push(InitializedStorage {
                storage,
                kind: OwnedStorageKind::AggregateOptional(optional),
            });
    }

    pub(super) fn register_array(
        &mut self,
        storage: StorageId,
        array: crate::identity::ArrayTypeId,
    ) {
        self.scopes
            .last_mut()
            .expect("an initialized local must belong to an active lexical scope")
            .initialized
            .push(InitializedStorage {
                storage,
                kind: OwnedStorageKind::Array(array),
            });
    }

    pub(super) fn for_current_scope(&self, span: Span) -> PlannedScopeExit {
        let retained = self
            .scopes
            .len()
            .checked_sub(1)
            .map(RetainedScopeDepth)
            .expect("a scope exit requires an active lexical scope");
        self.for_scopes_exiting_to(retained, span)
    }

    pub(super) fn for_all_scopes(&self, span: Span) -> PlannedScopeExit {
        self.for_scopes_exiting_to(RetainedScopeDepth(0), span)
    }

    /// Plan cleanup for scopes above `retained` without consuming planner state.
    pub(super) fn for_scopes_exiting_to(
        &self,
        retained: RetainedScopeDepth,
        span: Span,
    ) -> PlannedScopeExit {
        assert!(
            retained.0 <= self.scopes.len(),
            "retained cleanup depth must belong to the active scope stack"
        );
        PlannedScopeExit {
            cleanups: self
                .scopes
                .get(retained.0..)
                .expect("validated retained cleanup depth must slice active scopes")
                .iter()
                .rev()
                .flat_map(|scope| scope.initialized.iter().rev())
                .map(|local| local.cleanup(span))
                .collect(),
            storage: self
                .scopes
                .get(retained.0..)
                .expect("validated retained cleanup depth must slice active scopes")
                .iter()
                .rev()
                .flat_map(|scope| scope.storage.iter().rev())
                .copied()
                .collect(),
            span,
        }
    }

    pub(super) fn leave_scope(&mut self) {
        self.scopes
            .pop()
            .expect("leaving a scope requires an active lexical scope");
    }
}

impl InitializedStorage {
    fn cleanup(self, span: Span) -> PlannedCleanup {
        match self.kind {
            OwnedStorageKind::Inline(class) => PlannedCleanup::Inline(MirCleanup {
                destination: MirPlace::base(self.storage),
                target: class,
                span,
            }),
            OwnedStorageKind::Shared => PlannedCleanup::Shared(MirSharedRelease {
                owner: self.storage,
                span,
            }),
            OwnedStorageKind::ClassOptional(optional, class) => {
                PlannedCleanup::ClassOptional(crate::mir::MirClassOptionalCleanup {
                    optional,
                    destination: MirPlace::base(self.storage),
                    class,
                    span,
                })
            }
            OwnedStorageKind::OptionalShared(optional, target) => {
                PlannedCleanup::OptionalShared(crate::mir::MirOptionalSharedCleanup {
                    optional,
                    destination: MirPlace::base(self.storage),
                    target,
                    span,
                })
            }
            OwnedStorageKind::AggregateOptional(optional) => {
                PlannedCleanup::AggregateOptional(crate::mir::MirAggregateOptionalCleanup {
                    optional,
                    destination: MirPlace::base(self.storage),
                    span,
                })
            }
            OwnedStorageKind::Array(array) => PlannedCleanup::Array {
                storage: self.storage,
                array,
                span,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        identity::{CallableId, FunctionId},
        source::SourceDatabase,
    };

    #[test]
    fn plans_inner_scopes_and_locals_in_reverse_without_consuming_state() {
        let callable = CallableId::Function(FunctionId::new(0));
        let outer = StorageId::new(callable, 0);
        let first_inner = StorageId::new(callable, 1);
        let second_inner = StorageId::new(callable, 2);
        let mut sources = SourceDatabase::new();
        let source = sources.add("test.nif", "0123456789");
        let span = sources.get(source).unwrap().span(1, 5).unwrap();
        let mut planner = CleanupPlanner::new();

        planner.enter_scope();
        planner.register_storage(outer);
        planner.register_owned(outer, ClassId::new(0));
        let retain_outer = planner.retained_scope_depth();
        planner.enter_scope();
        planner.register_storage(first_inner);
        planner.register_storage(second_inner);
        planner.register_owned(first_inner, ClassId::new(1));
        planner.register_owned(second_inner, ClassId::new(2));

        let all = planner.for_all_scopes(span);
        assert_eq!(
            all.cleanups
                .iter()
                .map(|cleanup| match cleanup {
                    PlannedCleanup::Inline(cleanup) =>
                        cleanup.destination.base.expect_local_storage(),
                    PlannedCleanup::Shared(release) => release.owner,
                    PlannedCleanup::ClassOptional(cleanup) => {
                        cleanup.destination.base.expect_local_storage()
                    }
                    PlannedCleanup::OptionalShared(cleanup) => {
                        cleanup.destination.base.expect_local_storage()
                    }
                    PlannedCleanup::AggregateOptional(cleanup) => {
                        cleanup.destination.base.expect_local_storage()
                    }
                    PlannedCleanup::Array { storage, .. } => *storage,
                })
                .collect::<Vec<_>>(),
            [second_inner, first_inner, outer]
        );
        assert_eq!(all.storage, [second_inner, first_inner, outer]);
        assert_eq!(planner.for_current_scope(span).cleanups.len(), 2);
        assert_eq!(planner.for_current_scope(span).storage.len(), 2);
        let targeted = planner.for_scopes_exiting_to(retain_outer, span);
        let repeated = planner.for_scopes_exiting_to(retain_outer, span);
        assert_eq!(targeted.storage, [second_inner, first_inner]);
        assert_eq!(repeated.storage, targeted.storage);
        planner.leave_scope();
        let outer_exit = planner.for_current_scope(span);
        assert_eq!(
            match &outer_exit.cleanups[0] {
                PlannedCleanup::Inline(cleanup) => cleanup.destination.base.expect_local_storage(),
                PlannedCleanup::Shared(release) => release.owner,
                PlannedCleanup::ClassOptional(cleanup) =>
                    cleanup.destination.base.expect_local_storage(),
                PlannedCleanup::OptionalShared(cleanup) =>
                    cleanup.destination.base.expect_local_storage(),
                PlannedCleanup::AggregateOptional(cleanup) =>
                    cleanup.destination.base.expect_local_storage(),
                PlannedCleanup::Array { storage, .. } => *storage,
            },
            outer
        );
        assert_eq!(outer_exit.storage, [outer]);
    }

    #[test]
    fn targeted_cleanup_can_retain_zero_one_or_every_active_scope() {
        let callable = CallableId::Function(FunctionId::new(0));
        let outer = StorageId::new(callable, 0);
        let middle = StorageId::new(callable, 1);
        let inner = StorageId::new(callable, 2);
        let mut sources = SourceDatabase::new();
        let source = sources.add("test.ska", "");
        let span = sources.get(source).unwrap().span(0, 0).unwrap();
        let mut planner = CleanupPlanner::new();

        planner.enter_scope();
        planner.register_storage(outer);
        let retain_outer = planner.retained_scope_depth();
        planner.enter_scope();
        planner.register_storage(middle);
        planner.enter_scope();
        planner.register_storage(inner);
        let retain_all = planner.retained_scope_depth();

        assert_eq!(
            planner
                .for_scopes_exiting_to(RetainedScopeDepth(0), span)
                .storage,
            [inner, middle, outer]
        );
        assert_eq!(
            planner.for_scopes_exiting_to(retain_outer, span).storage,
            [inner, middle]
        );
        assert!(planner
            .for_scopes_exiting_to(retain_all, span)
            .storage
            .is_empty());
    }
}
