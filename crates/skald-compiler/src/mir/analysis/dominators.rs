//! Entry-rooted dominance for one immutable callable CFG snapshot.

use std::collections::HashMap;

use super::MirCfgTopology;
use crate::mir::BlockId;

/// Precomputed dominance membership for one [`MirCfgTopology`].
///
/// Known blocks dominate themselves. For every other pair, dominance is
/// defined only inside the entry-reachable component. Block ordinals follow
/// snapshot order and never depend on the numeric payload of a block ID.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MirDominators {
    block_ordinals: HashMap<BlockId, usize>,
    membership: Vec<Vec<bool>>,
}

impl MirDominators {
    pub(crate) fn for_topology(topology: &MirCfgTopology) -> Self {
        let blocks = topology
            .blocks()
            .iter()
            .filter_map(|facts| topology.block(facts.block()).map(|_| facts.block()))
            .collect::<Vec<_>>();
        let block_ordinals = blocks
            .iter()
            .copied()
            .enumerate()
            .map(|(ordinal, block)| (block, ordinal))
            .collect::<HashMap<_, _>>();
        let entry = topology.entry();
        let reachable = topology.entry_reachable();
        let mut membership = vec![vec![false; blocks.len()]; blocks.len()];

        for (target_ordinal, target) in blocks.iter().copied().enumerate() {
            if Some(target) == entry || !reachable.contains(&target) {
                membership[target_ordinal][target_ordinal] = true;
            } else {
                for source in reachable {
                    if let Some(source_ordinal) = block_ordinals.get(source) {
                        membership[target_ordinal][*source_ordinal] = true;
                    }
                }
            }
        }

        let mut changed = true;
        while changed {
            changed = false;
            for (target_ordinal, target) in blocks.iter().copied().enumerate() {
                if Some(target) == entry || !reachable.contains(&target) {
                    continue;
                }

                let mut predecessors = topology
                    .block(target)
                    .expect("known dominance block has topology facts")
                    .predecessor_blocks()
                    .iter()
                    .filter_map(|block| block_ordinals.get(block).copied())
                    .filter(|ordinal| reachable.contains(&blocks[*ordinal]));
                let mut updated = if let Some(first) = predecessors.next() {
                    let mut intersection = membership[first].clone();
                    for predecessor in predecessors {
                        for (member, predecessor_member) in
                            intersection.iter_mut().zip(&membership[predecessor])
                        {
                            *member &= *predecessor_member;
                        }
                    }
                    intersection
                } else {
                    vec![false; blocks.len()]
                };
                updated[target_ordinal] = true;
                if updated != membership[target_ordinal] {
                    membership[target_ordinal] = updated;
                    changed = true;
                }
            }
        }

        Self {
            block_ordinals,
            membership,
        }
    }

    /// Returns whether `dominator` dominates `target` in this snapshot.
    ///
    /// Unknown, foreign, ambiguous, and structurally invalid identities fail
    /// closed, including when both arguments contain the same invalid ID.
    pub(crate) fn dominates(&self, dominator: BlockId, target: BlockId) -> bool {
        let Some(dominator) = self.block_ordinals.get(&dominator).copied() else {
            return false;
        };
        let Some(target) = self.block_ordinals.get(&target).copied() else {
            return false;
        };
        self.membership[target][dominator]
    }
}

#[cfg(test)]
mod tests;
