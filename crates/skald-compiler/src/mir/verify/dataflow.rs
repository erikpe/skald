//! Deterministic scheduling for finite forward MIR dataflow analyses.

use std::collections::VecDeque;

use crate::{
    identity::CallableId,
    mir::{BlockId, MirBasicBlock},
};

/// Stores one incoming state per block and schedules changed states once.
///
/// Domain owners retain their own transfer and merge semantics. This helper
/// only centralizes deterministic queueing, duplicate suppression, malformed
/// target bounds, and table-ordered seeding of disconnected CFG components.
pub(super) struct ForwardDataflow<State> {
    callable: CallableId,
    incoming: Vec<Option<State>>,
    pending: VecDeque<BlockId>,
    queued: Vec<bool>,
}

impl<State> ForwardDataflow<State> {
    pub(super) fn new(callable: impl Into<CallableId>, block_count: usize) -> Self {
        Self {
            callable: callable.into(),
            incoming: (0..block_count).map(|_| None).collect(),
            pending: VecDeque::new(),
            queued: vec![false; block_count],
        }
    }

    pub(super) fn seed(&mut self, block: BlockId, state: State) -> bool {
        if block.callable() != self.callable {
            return false;
        }
        let Some(slot) = self.incoming.get_mut(block.index()) else {
            return false;
        };
        if slot.is_some() {
            return false;
        }
        *slot = Some(state);
        self.schedule(block);
        true
    }

    pub(super) fn seed_next_component(&mut self, blocks: &[MirBasicBlock], state: State) -> bool {
        let Some(block) = blocks.iter().find(|block| {
            self.incoming
                .get(block.id.index())
                .is_some_and(Option::is_none)
        }) else {
            return false;
        };
        self.seed(block.id, state)
    }

    pub(super) fn pop(&mut self) -> Option<(BlockId, State)>
    where
        State: Clone,
    {
        let block = self.pending.pop_front()?;
        self.queued[block.index()] = false;
        self.incoming[block.index()]
            .as_ref()
            .cloned()
            .map(|state| (block, state))
    }

    pub(super) fn state(&self, block: BlockId) -> Option<&State> {
        self.incoming.get(block.index()).and_then(Option::as_ref)
    }

    pub(super) fn merge(
        &mut self,
        target: BlockId,
        state: &State,
        merge: impl FnOnce(&mut State, &State) -> bool,
    ) where
        State: Clone,
    {
        if target.callable() != self.callable {
            return;
        }
        let Some(slot) = self.incoming.get_mut(target.index()) else {
            return;
        };
        let changed = match slot {
            None => {
                *slot = Some(state.clone());
                true
            }
            Some(existing) => merge(existing, state),
        };
        if changed {
            self.schedule(target);
        }
    }

    fn schedule(&mut self, block: BlockId) {
        let Some(queued) = self.queued.get_mut(block.index()) else {
            return;
        };
        if !*queued {
            *queued = true;
            self.pending.push_back(block);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::identity::FunctionId;

    use super::*;

    #[test]
    fn schedules_changed_blocks_once_and_seeds_components_in_table_order() {
        let function = FunctionId::new(0);
        let first = BlockId::new(function, 0);
        let second = BlockId::new(function, 1);
        let mut sources = crate::source::SourceDatabase::new();
        let source = sources.add("test.ska", "");
        let span = sources.get(source).unwrap().span(0, 0).unwrap();
        let blocks = [
            MirBasicBlock {
                id: first,
                instructions: vec![],
                terminator: None,
                span,
            },
            MirBasicBlock {
                id: second,
                instructions: vec![],
                terminator: None,
                span,
            },
        ];
        let mut flow = ForwardDataflow::new(function, blocks.len());
        assert!(flow.seed(first, 1_u8));
        flow.merge(first, &2, |existing, incoming| {
            *existing = *incoming;
            true
        });
        assert_eq!(flow.pop(), Some((first, 2)));
        assert_eq!(flow.pop(), None);
        assert!(flow.seed_next_component(&blocks, 3));
        assert_eq!(flow.pop(), Some((second, 3)));
        assert!(!flow.seed_next_component(&blocks, 4));
    }
}
