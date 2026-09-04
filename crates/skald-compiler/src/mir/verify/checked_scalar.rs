//! Shared graph and carrier queries for verified checked-scalar diamonds.

use std::collections::{HashMap, HashSet};

use super::super::model::{
    BlockId, MirDefinitionRef, MirInstruction, MirPlace, MirRvalueKind, StorageId,
};

pub(crate) fn predecessors(function: MirDefinitionRef<'_>) -> HashMap<BlockId, HashSet<BlockId>> {
    let mut predecessors = HashMap::<_, HashSet<_>>::new();
    for block in &function.body().blocks {
        if let Some(terminator) = &block.terminator {
            for successor in terminator.successors() {
                predecessors.entry(successor).or_default().insert(block.id);
            }
        }
    }
    predecessors
}

pub(super) fn storage_writes(function: MirDefinitionRef<'_>, storage: StorageId) -> Vec<BlockId> {
    function
        .body()
        .blocks
        .iter()
        .flat_map(|block| {
            block.instructions.iter().filter_map(move |instruction| {
                matches!(
                    instruction,
                    MirInstruction::Store(store)
                        if store.destination == MirPlace::base(storage)
                )
                .then_some(block.id)
            })
        })
        .collect()
}

pub(crate) fn dominates(
    function: MirDefinitionRef<'_>,
    dominator: BlockId,
    target: BlockId,
) -> bool {
    dominator == target
        || (reachable(function, target, None) && !reachable(function, target, Some(dominator)))
}

fn reachable(function: MirDefinitionRef<'_>, target: BlockId, excluded: Option<BlockId>) -> bool {
    let entry = function.body().entry;
    if excluded == Some(entry) {
        return false;
    }
    let mut pending = vec![entry];
    let mut visited = HashSet::new();
    while let Some(block) = pending.pop() {
        if block == target {
            return true;
        }
        if !visited.insert(block) {
            continue;
        }
        let Some(block) = function.block(block) else {
            continue;
        };
        if let Some(terminator) = &block.terminator {
            pending.extend(
                terminator
                    .successors()
                    .filter(|successor| Some(*successor) != excluded),
            );
        }
    }
    false
}

pub(super) fn is_exact_load(rvalue: &MirRvalueKind, storage: StorageId) -> bool {
    matches!(rvalue, MirRvalueKind::Load(place) if *place == MirPlace::base(storage))
}
