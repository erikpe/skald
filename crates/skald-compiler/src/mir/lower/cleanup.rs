//! Lexical ownership state and cleanup planning for MIR control-flow edges.

use crate::{identity::ClassId, source::Span};

use super::{MirCleanup, MirPlace, StorageId};

/// Owning storage whose initialization completed on the current path.
#[derive(Clone, Copy)]
struct InitializedStorage {
    storage: StorageId,
    class: ClassId,
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
            .push(InitializedStorage { storage, class });
    }

    pub(super) fn for_current_scope(&self, span: Span) -> Vec<MirCleanup> {
        self.scopes
            .last()
            .expect("a scope exit requires an active lexical scope")
            .iter()
            .rev()
            .map(|local| local.cleanup(span))
            .collect()
    }

    pub(super) fn for_all_scopes(&self, span: Span) -> Vec<MirCleanup> {
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
    fn cleanup(self, span: Span) -> MirCleanup {
        MirCleanup {
            destination: MirPlace::base(self.storage),
            target: self.class,
            span,
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
                .map(|cleanup| cleanup.destination.base.storage())
                .collect::<Vec<_>>(),
            [second_inner, first_inner, outer]
        );
        assert_eq!(planner.for_current_scope(span).len(), 2);
        assert_eq!(planner.for_current_scope(span).len(), 2);
        planner.leave_scope();
        assert_eq!(
            planner.for_current_scope(span)[0]
                .destination
                .base
                .storage(),
            outer
        );
    }
}
