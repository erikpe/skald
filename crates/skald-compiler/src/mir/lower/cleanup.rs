//! Lexical ownership state and cleanup planning for MIR control-flow edges.

use crate::{identity::ClassId, source::Span};

use super::{
    BodyLowerer, MirCheckedViewEnd, MirCleanup, MirEndFullExpression, MirInstruction, MirPlace,
    MirSharedRelease, StorageId,
};

impl BodyLowerer<'_> {
    pub(super) fn finish_full_expression(&mut self, span: Span) {
        let checked_views: Vec<_> = self.full_expression_checked_views.drain(..).rev().collect();
        for carrier in checked_views {
            self.emit(MirInstruction::EndCheckedView(MirCheckedViewEnd {
                carrier,
                span,
            }));
        }
        if self.full_expression_temporaries.is_empty()
            && self.full_expression_shared_temporaries.is_empty()
            && !self.full_expression_has_shared_effect
        {
            return;
        }
        let temporaries = self
            .full_expression_temporaries
            .drain(..)
            .rev()
            .map(|mut cleanup| {
                cleanup.span = span;
                cleanup
            })
            .collect();
        let shared_temporaries: Vec<_> = self
            .full_expression_shared_temporaries
            .drain(..)
            .rev()
            .collect();
        for owner in shared_temporaries {
            self.emit(MirInstruction::SharedRelease(MirSharedRelease {
                owner,
                span,
            }));
        }
        self.emit(MirInstruction::EndFullExpression(MirEndFullExpression {
            temporaries,
            span,
        }));
        self.full_expression_has_shared_effect = false;
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
}

pub(super) enum PlannedCleanup {
    Inline(MirCleanup),
    Shared(MirSharedRelease),
}

impl PlannedCleanup {
    pub(super) fn into_instruction(self) -> MirInstruction {
        match self {
            Self::Inline(cleanup) => MirInstruction::Cleanup(cleanup),
            Self::Shared(release) => MirInstruction::SharedRelease(release),
        }
    }
}

/// Tracks lexical ownership independently from expression lowering.
///
/// Planning does not consume state: one source scope may have several outgoing
/// CFG edges, and each edge needs the same cleanup sequence. Leaving the source
/// scope is the only operation that discards its registrations.
pub(super) struct CleanupPlanner {
    scopes: Vec<Vec<InitializedStorage>>,
}

impl CleanupPlanner {
    pub(super) const fn new() -> Self {
        Self { scopes: Vec::new() }
    }

    pub(super) fn enter_scope(&mut self) {
        self.scopes.push(Vec::new());
    }

    pub(super) fn register_owned(&mut self, storage: StorageId, class: ClassId) {
        self.scopes
            .last_mut()
            .expect("an initialized local must belong to an active lexical scope")
            .push(InitializedStorage {
                storage,
                kind: OwnedStorageKind::Inline(class),
            });
    }

    pub(super) fn register_shared(&mut self, storage: StorageId) {
        self.scopes
            .last_mut()
            .expect("an initialized local must belong to an active lexical scope")
            .push(InitializedStorage {
                storage,
                kind: OwnedStorageKind::Shared,
            });
    }

    pub(super) fn for_current_scope(&self, span: Span) -> Vec<PlannedCleanup> {
        self.scopes
            .last()
            .expect("a scope exit requires an active lexical scope")
            .iter()
            .rev()
            .map(|local| local.cleanup(span))
            .collect()
    }

    pub(super) fn for_all_scopes(&self, span: Span) -> Vec<PlannedCleanup> {
        self.scopes
            .iter()
            .rev()
            .flat_map(|scope| scope.iter().rev())
            .map(|local| local.cleanup(span))
            .collect()
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
        planner.register_owned(outer, ClassId::new(0));
        planner.enter_scope();
        planner.register_owned(first_inner, ClassId::new(1));
        planner.register_owned(second_inner, ClassId::new(2));

        let all = planner.for_all_scopes(span);
        assert_eq!(
            all.iter()
                .map(|cleanup| match cleanup {
                    PlannedCleanup::Inline(cleanup) => cleanup.destination.base.storage(),
                    PlannedCleanup::Shared(release) => release.owner,
                })
                .collect::<Vec<_>>(),
            [second_inner, first_inner, outer]
        );
        assert_eq!(planner.for_current_scope(span).len(), 2);
        assert_eq!(planner.for_current_scope(span).len(), 2);
        planner.leave_scope();
        assert_eq!(
            match &planner.for_current_scope(span)[0] {
                PlannedCleanup::Inline(cleanup) => cleanup.destination.base.storage(),
                PlannedCleanup::Shared(release) => release.owner,
            },
            outer
        );
    }
}
