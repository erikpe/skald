//! Deterministic structural control-flow facts for one callable snapshot.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crate::identity::CallableId;

use super::super::{BlockId, MirDefinitionRef, MirTerminator};

/// One executable successor occurrence in a callable-local terminator.
///
/// `successor_index` is the stable position in [`MirTerminator::successors`]
/// semantic order. Keeping occurrences distinct preserves parallel edges from
/// conditionals whose targets happen to be equal.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct MirCfgEdge {
    source: BlockId,
    target: BlockId,
    successor_index: usize,
}

impl MirCfgEdge {
    pub(crate) const fn new(source: BlockId, target: BlockId, successor_index: usize) -> Self {
        Self {
            source,
            target,
            successor_index,
        }
    }

    pub(crate) const fn source(self) -> BlockId {
        self.source
    }

    pub(crate) const fn target(self) -> BlockId {
        self.target
    }

    pub(crate) const fn successor_index(self) -> usize {
        self.successor_index
    }
}

/// Ordered edges and set-oriented predecessor facts for one declared block.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MirCfgBlockTopology {
    block: BlockId,
    successors: Vec<BlockId>,
    successor_edges: Vec<MirCfgEdge>,
    predecessor_edges: Vec<MirCfgEdge>,
    predecessor_blocks: BTreeSet<BlockId>,
}

impl MirCfgBlockTopology {
    pub(crate) const fn block(&self) -> BlockId {
        self.block
    }

    /// Successor targets in terminator semantic order, including duplicates.
    pub(crate) fn successors(&self) -> &[BlockId] {
        &self.successors
    }

    pub(crate) fn successor_edges(&self) -> &[MirCfgEdge] {
        &self.successor_edges
    }

    pub(crate) fn predecessor_edges(&self) -> &[MirCfgEdge] {
        &self.predecessor_edges
    }

    /// Unique predecessor blocks, independent of parallel edge count.
    pub(crate) fn predecessor_blocks(&self) -> &BTreeSet<BlockId> {
        &self.predecessor_blocks
    }
}

/// Tolerant structural facts for one immutable callable CFG snapshot.
///
/// Construction records every declared edge occurrence. Block-oriented and
/// traversal queries admit only unique local declarations, so malformed MIR
/// cannot redirect analysis through a foreign, duplicate, or misindexed ID.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MirCfgTopology {
    blocks: Vec<MirCfgBlockTopology>,
    block_indices: BTreeMap<BlockId, usize>,
    edges: Vec<MirCfgEdge>,
    predecessor_blocks: BTreeMap<BlockId, BTreeSet<BlockId>>,
    entry: Option<BlockId>,
    entry_reachable: BTreeSet<BlockId>,
}

impl MirCfgTopology {
    /// Builds tolerant facts directly from a dense MIR definition.
    pub(crate) fn for_definition(definition: MirDefinitionRef<'_>) -> Self {
        let callable = definition.callable();
        let blocks = definition
            .body()
            .blocks
            .iter()
            .enumerate()
            .map(|(position, block)| CfgBlockInput {
                block: block.id,
                successors: block
                    .terminator
                    .iter()
                    .flat_map(MirTerminator::successors)
                    .collect(),
                structurally_local: block.id.callable() == callable && block.id.index() == position,
            })
            .collect();
        Self::build(callable, definition.body().entry, blocks)
    }

    /// Builds facts from an explicitly ordered, potentially sparse MIR
    /// snapshot. Strict validation remains the caller's responsibility.
    pub(in crate::mir) fn from_ordered_successors<Blocks, Successors>(
        callable: CallableId,
        entry: BlockId,
        blocks: Blocks,
    ) -> Self
    where
        Blocks: IntoIterator<Item = (BlockId, Successors)>,
        Successors: IntoIterator<Item = BlockId>,
    {
        let blocks = blocks
            .into_iter()
            .map(|(block, successors)| CfgBlockInput {
                block,
                successors: successors.into_iter().collect(),
                structurally_local: block.callable() == callable,
            })
            .collect();
        Self::build(callable, entry, blocks)
    }

    fn build(callable: CallableId, entry: BlockId, inputs: Vec<CfgBlockInput>) -> Self {
        let mut occurrences = BTreeMap::<BlockId, usize>::new();
        for input in &inputs {
            *occurrences.entry(input.block).or_default() += 1;
        }

        let mut block_indices = BTreeMap::new();
        for (position, input) in inputs.iter().enumerate() {
            if input.structurally_local && occurrences.get(&input.block) == Some(&1) {
                block_indices.insert(input.block, position);
            }
        }

        let mut edges = Vec::new();
        let mut blocks = inputs
            .into_iter()
            .map(|input| {
                let successor_edges = input
                    .successors
                    .iter()
                    .copied()
                    .enumerate()
                    .map(|(successor_index, target)| {
                        MirCfgEdge::new(input.block, target, successor_index)
                    })
                    .collect::<Vec<_>>();
                edges.extend(successor_edges.iter().copied());
                MirCfgBlockTopology {
                    block: input.block,
                    successors: input.successors,
                    successor_edges,
                    predecessor_edges: Vec::new(),
                    predecessor_blocks: BTreeSet::new(),
                }
            })
            .collect::<Vec<_>>();

        let mut predecessor_blocks = BTreeMap::<BlockId, BTreeSet<BlockId>>::new();
        for edge in &edges {
            predecessor_blocks
                .entry(edge.target())
                .or_default()
                .insert(edge.source());
            if let Some(target) = block_indices.get(&edge.target()).copied() {
                blocks[target].predecessor_edges.push(*edge);
                if block_indices.contains_key(&edge.source()) {
                    blocks[target].predecessor_blocks.insert(edge.source());
                }
            }
        }

        let valid_entry = block_indices.contains_key(&entry).then_some(entry);
        let mut topology = Self {
            blocks,
            block_indices,
            edges,
            predecessor_blocks,
            entry: valid_entry,
            entry_reachable: BTreeSet::new(),
        };
        topology.entry_reachable = topology.reachable_from([entry]);

        debug_assert!(topology
            .block_indices
            .keys()
            .all(|block| block.callable() == callable));
        topology
    }

    pub(crate) fn edges(&self) -> &[MirCfgEdge] {
        &self.edges
    }

    pub(crate) fn blocks(&self) -> &[MirCfgBlockTopology] {
        &self.blocks
    }

    pub(crate) fn block(&self, block: BlockId) -> Option<&MirCfgBlockTopology> {
        self.block_indices
            .get(&block)
            .and_then(|position| self.blocks.get(*position))
    }

    /// Unique sources for any target named by a declared edge.
    ///
    /// Unlike [`Self::block`], this can return facts for an undeclared target.
    /// Verification uses that distinction to preserve diagnostics when valid
    /// and invalid target references coexist in malformed MIR.
    pub(crate) fn predecessor_blocks(&self, block: BlockId) -> Option<&BTreeSet<BlockId>> {
        self.predecessor_blocks.get(&block)
    }

    pub(super) const fn entry(&self) -> Option<BlockId> {
        self.entry
    }

    pub(crate) fn entry_reachable(&self) -> &BTreeSet<BlockId> {
        &self.entry_reachable
    }

    /// Computes closure from caller-selected roots while ignoring identities
    /// which are not unambiguous local declarations in this snapshot.
    pub(crate) fn reachable_from(
        &self,
        roots: impl IntoIterator<Item = BlockId>,
    ) -> BTreeSet<BlockId> {
        let mut reachable = BTreeSet::new();
        let mut pending = VecDeque::from_iter(roots);
        while let Some(block) = pending.pop_front() {
            let Some(facts) = self.block(block) else {
                continue;
            };
            if !reachable.insert(block) {
                continue;
            }
            pending.extend(
                facts
                    .successors()
                    .iter()
                    .copied()
                    .filter(|successor| self.block(*successor).is_some()),
            );
        }
        reachable
    }

    pub(in crate::mir) fn into_blocks(self) -> Vec<MirCfgBlockTopology> {
        self.blocks
    }
}

struct CfgBlockInput {
    block: BlockId,
    successors: Vec<BlockId>,
    structurally_local: bool,
}

#[cfg(test)]
mod tests;
