//! One entry-rooted reachability/dominance construction per immutable session.
use super::model::{GraphDescription, GraphView};
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct GraphSession<'a, G: GraphView> {
    pub(super) owner: &'a G,
    pub(super) reachable: Vec<bool>,
    pub(super) dominators: Vec<Vec<bool>>,
    pub(super) predecessors: Vec<Vec<(usize, usize)>>,
}
#[cfg_attr(not(test), allow(dead_code))]
impl<G: GraphView> GraphSession<'_, G> {
    pub(in crate::backend) fn owner(&self) -> &G {
        self.owner
    }
    pub(in crate::backend) fn reachable(&self, block: usize) -> Option<bool> {
        self.reachable.get(block).copied()
    }
    /// Cross-block queries involving unreachable blocks deliberately return unknown.
    pub(in crate::backend) fn dominates(
        &self,
        definition: usize,
        use_block: usize,
    ) -> Option<bool> {
        let def = *self.reachable.get(definition)?;
        let used = *self.reachable.get(use_block)?;
        if definition == use_block {
            return Some(true);
        }
        if !def || !used {
            return None;
        }
        Some(self.dominators[use_block][definition])
    }
    pub(in crate::backend) fn predecessors(&self, block: usize) -> Option<&[(usize, usize)]> {
        self.predecessors.get(block).map(Vec::as_slice)
    }
}
#[cfg_attr(not(test), allow(dead_code))]
pub(super) fn analyze<'a, G: GraphView>(
    owner: &'a G,
    graph: &GraphDescription<G::Type>,
    entry: usize,
) -> GraphSession<'a, G> {
    let n = graph.blocks.len();
    let mut predecessors = vec![Vec::new(); n];
    let mut successors = vec![Vec::new(); n];
    for (block, data) in graph.blocks.iter().enumerate() {
        for (slot, edge) in data
            .terminal
            .as_ref()
            .expect("structure checked")
            .1
            .iter()
            .enumerate()
        {
            successors[block].push(edge.target);
            predecessors[edge.target].push((block, slot));
        }
    }
    let mut reachable = vec![false; n];
    let mut work = vec![entry];
    while let Some(block) = work.pop() {
        if std::mem::replace(&mut reachable[block], true) {
            continue;
        }
        work.extend(successors[block].iter().copied());
    }
    let mut dominators = vec![reachable.clone(); n];
    dominators[entry].fill(false);
    dominators[entry][entry] = true;
    loop {
        let mut changed = false;
        for block in 0..n {
            if !reachable[block] || block == entry {
                continue;
            }
            let mut row = reachable.clone();
            for &(predecessor, _) in &predecessors[block] {
                if reachable[predecessor] {
                    for (bit, prior) in row.iter_mut().zip(&dominators[predecessor]) {
                        *bit &= *prior;
                    }
                }
            }
            row[block] = true;
            if row != dominators[block] {
                dominators[block] = row;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    GraphSession {
        owner,
        reachable,
        dominators,
        predecessors,
    }
}
