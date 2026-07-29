//! Lexical ownership state and cleanup planning for MIR control-flow edges.

use crate::{identity::ClassId, source::Span};

use super::{
    BodyLowerer, MirArrayInstruction, MirCheckedViewEnd, MirCleanup, MirEndFullExpression,
    MirInstruction, MirPlace, MirSharedRelease, StorageId,
};

impl BodyLowerer<'_> {
    pub(super) fn finish_full_expression(&mut self, span: Span) {
        self.end_optional_views_from(0, span);
        let checked_views: Vec<_> = self.full_expression_checked_views.drain(..).rev().collect();
        for carrier in checked_views {
            self.emit(MirInstruction::EndCheckedView(MirCheckedViewEnd {
                carrier,
                span,
            }));
        }
        if self.full_expression_temporaries.is_empty()
            && !self.full_expression_has_shared_effect
            && self.full_expression_storage.is_empty()
        {
            return;
        }
        let temporaries: Vec<_> = self.full_expression_temporaries.drain(..).rev().collect();
        let mut inline = Vec::new();
        for temporary in temporaries {
            match temporary {
                super::FullExpressionTemporary::Inline(mut cleanup) => {
                    cleanup.span = span;
                    inline.push(cleanup);
                }
                super::FullExpressionTemporary::Shared(owner) => {
                    if !inline.is_empty() {
                        self.emit(MirInstruction::EndFullExpression(MirEndFullExpression {
                            temporaries: std::mem::take(&mut inline),
                            span,
                        }));
                    }
                    self.emit(MirInstruction::SharedRelease(MirSharedRelease {
                        owner,
                        span,
                    }));
                }
                super::FullExpressionTemporary::ClassOptional(cleanup) => {
                    if !inline.is_empty() {
                        self.emit(MirInstruction::EndFullExpression(MirEndFullExpression {
                            temporaries: std::mem::take(&mut inline),
                            span,
                        }));
                    }
                    self.emit_class_optional_cleanup(cleanup);
                }
                super::FullExpressionTemporary::OptionalShared(cleanup) => {
                    if !inline.is_empty() {
                        self.emit(MirInstruction::EndFullExpression(MirEndFullExpression {
                            temporaries: std::mem::take(&mut inline),
                            span,
                        }));
                    }
                    self.emit(MirInstruction::OptionalSharedCleanup(cleanup));
                }
                super::FullExpressionTemporary::Array { storage, array } => {
                    if !inline.is_empty() {
                        self.emit(MirInstruction::EndFullExpression(MirEndFullExpression {
                            temporaries: std::mem::take(&mut inline),
                            span,
                        }));
                    }
                    self.emit(MirInstruction::Array(MirArrayInstruction::Release {
                        owner: MirPlace::base(storage),
                        array,
                        span,
                    }));
                }
                super::FullExpressionTemporary::ArrayAnchor(anchor) => {
                    if !inline.is_empty() {
                        self.emit(MirInstruction::EndFullExpression(MirEndFullExpression {
                            temporaries: std::mem::take(&mut inline),
                            span,
                        }));
                    }
                    self.emit(MirInstruction::Array(MirArrayInstruction::AnchorEnd {
                        anchor,
                        span,
                    }));
                }
            }
        }
        self.emit(MirInstruction::EndFullExpression(MirEndFullExpression {
            temporaries: inline,
            span,
        }));
        self.full_expression_has_shared_effect = false;
        let storage: Vec<_> = self.full_expression_storage.drain(..).rev().collect();
        for storage in storage {
            self.end_storage_lifetime(storage, span);
        }
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
    ClassOptional(ClassId),
    OptionalShared(crate::mir::MirSharedTarget),
    Array(crate::identity::ArrayTypeId),
}

pub(super) enum PlannedCleanup {
    Inline(MirCleanup),
    Shared(MirSharedRelease),
    ClassOptional(crate::mir::MirClassOptionalCleanup),
    OptionalShared(crate::mir::MirOptionalSharedCleanup),
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
        matches!(self, Self::ClassOptional(_))
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

    pub(super) fn register_class_optional(&mut self, storage: StorageId, class: ClassId) {
        self.scopes
            .last_mut()
            .expect("an initialized local must belong to an active lexical scope")
            .initialized
            .push(InitializedStorage {
                storage,
                kind: OwnedStorageKind::ClassOptional(class),
            });
    }

    pub(super) fn register_optional_shared(
        &mut self,
        storage: StorageId,
        target: crate::mir::MirSharedTarget,
    ) {
        self.scopes
            .last_mut()
            .expect("an initialized local must belong to an active lexical scope")
            .initialized
            .push(InitializedStorage {
                storage,
                kind: OwnedStorageKind::OptionalShared(target),
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
        let scope = self
            .scopes
            .last()
            .expect("a scope exit requires an active lexical scope");
        PlannedScopeExit {
            cleanups: scope
                .initialized
                .iter()
                .rev()
                .map(|local| local.cleanup(span))
                .collect(),
            storage: scope.storage.iter().rev().copied().collect(),
            span,
        }
    }

    pub(super) fn for_all_scopes(&self, span: Span) -> PlannedScopeExit {
        PlannedScopeExit {
            cleanups: self
                .scopes
                .iter()
                .rev()
                .flat_map(|scope| scope.initialized.iter().rev())
                .map(|local| local.cleanup(span))
                .collect(),
            storage: self
                .scopes
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
            OwnedStorageKind::ClassOptional(class) => {
                PlannedCleanup::ClassOptional(crate::mir::MirClassOptionalCleanup {
                    destination: MirPlace::base(self.storage),
                    class,
                    span,
                })
            }
            OwnedStorageKind::OptionalShared(target) => {
                PlannedCleanup::OptionalShared(crate::mir::MirOptionalSharedCleanup {
                    destination: MirPlace::base(self.storage),
                    target,
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
                    PlannedCleanup::Inline(cleanup) => cleanup.destination.base.storage(),
                    PlannedCleanup::Shared(release) => release.owner,
                    PlannedCleanup::ClassOptional(cleanup) => {
                        cleanup.destination.base.storage()
                    }
                    PlannedCleanup::OptionalShared(cleanup) => {
                        cleanup.destination.base.storage()
                    }
                    PlannedCleanup::Array { storage, .. } => *storage,
                })
                .collect::<Vec<_>>(),
            [second_inner, first_inner, outer]
        );
        assert_eq!(all.storage, [second_inner, first_inner, outer]);
        assert_eq!(planner.for_current_scope(span).cleanups.len(), 2);
        assert_eq!(planner.for_current_scope(span).storage.len(), 2);
        planner.leave_scope();
        let outer_exit = planner.for_current_scope(span);
        assert_eq!(
            match &outer_exit.cleanups[0] {
                PlannedCleanup::Inline(cleanup) => cleanup.destination.base.storage(),
                PlannedCleanup::Shared(release) => release.owner,
                PlannedCleanup::ClassOptional(cleanup) => cleanup.destination.base.storage(),
                PlannedCleanup::OptionalShared(cleanup) => cleanup.destination.base.storage(),
                PlannedCleanup::Array { storage, .. } => *storage,
            },
            outer
        );
        assert_eq!(outer_exit.storage, [outer]);
    }
}
