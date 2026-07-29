//! Structured loop targets retained during HIR-to-MIR lowering.

use crate::{identity::LoopId, mir::BlockId};

use super::cleanup::RetainedScopeDepth;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct LoopContext {
    loop_id: LoopId,
    exit_target: BlockId,
    latch_target: BlockId,
    retained_scope_depth: RetainedScopeDepth,
}

impl LoopContext {
    pub(super) fn new(
        loop_id: LoopId,
        exit_target: BlockId,
        latch_target: BlockId,
        retained_scope_depth: RetainedScopeDepth,
    ) -> Option<Self> {
        (exit_target.callable() == loop_id.callable()
            && latch_target.callable() == loop_id.callable())
        .then_some(Self {
            loop_id,
            exit_target,
            latch_target,
            retained_scope_depth,
        })
    }

    pub(super) const fn loop_id(self) -> LoopId {
        self.loop_id
    }

    pub(super) const fn exit_target(self) -> BlockId {
        self.exit_target
    }

    pub(super) const fn latch_target(self) -> BlockId {
        self.latch_target
    }

    pub(super) const fn retained_scope_depth(self) -> RetainedScopeDepth {
        self.retained_scope_depth
    }
}

#[derive(Default)]
pub(super) struct LoopContextStack {
    contexts: Vec<LoopContext>,
}

impl LoopContextStack {
    pub(super) const fn new() -> Self {
        Self {
            contexts: Vec::new(),
        }
    }

    pub(super) fn push(&mut self, context: LoopContext) {
        self.contexts.push(context);
    }

    pub(super) fn pop(&mut self, loop_id: LoopId) -> LoopContext {
        let context = self
            .contexts
            .pop()
            .expect("leaving a loop requires an active lowering context");
        assert_eq!(
            context.loop_id, loop_id,
            "loop lowering contexts must leave in lexical nesting order"
        );
        context
    }

    pub(super) fn find(&self, loop_id: LoopId) -> Option<LoopContext> {
        self.contexts
            .iter()
            .rev()
            .find(|context| context.loop_id == loop_id)
            .copied()
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        identity::{ClassId, FunctionId, MethodId},
        mir::BlockId,
    };

    use super::*;
    use crate::mir::lower::cleanup::CleanupPlanner;

    #[test]
    fn validates_owners_and_preserves_nested_loop_targets() {
        let function = FunctionId::new(0);
        let outer_id = LoopId::new(function, 0);
        let inner_id = LoopId::new(function, 1);
        let mut cleanup = CleanupPlanner::new();
        cleanup.enter_scope();
        let outer_depth = cleanup.retained_scope_depth();
        cleanup.enter_scope();
        let inner_depth = cleanup.retained_scope_depth();
        let outer = LoopContext::new(
            outer_id,
            BlockId::new(function, 4),
            BlockId::new(function, 3),
            outer_depth,
        )
        .unwrap();
        let inner = LoopContext::new(
            inner_id,
            BlockId::new(function, 8),
            BlockId::new(function, 7),
            inner_depth,
        )
        .unwrap();
        let foreign = MethodId::new(ClassId::new(0), 0);
        assert!(LoopContext::new(
            outer_id,
            BlockId::new(foreign, 0),
            BlockId::new(function, 0),
            outer_depth,
        )
        .is_none());

        let mut stack = LoopContextStack::new();
        stack.push(outer);
        stack.push(inner);
        assert_eq!(
            stack.find(inner_id).unwrap().latch_target(),
            inner.latch_target()
        );
        assert_eq!(
            stack.find(outer_id).unwrap().exit_target(),
            outer.exit_target()
        );
        assert_eq!(stack.pop(inner_id), inner);
        assert_eq!(stack.pop(outer_id), outer);
    }
}
