//! Dynamic storage-lifetime epoch verification.

use std::collections::{BTreeSet, HashSet, VecDeque};

use crate::mir::{MirDefinitionRef, MirInstruction, MirStorageKind, MirTerminator, StorageId};

use super::context::Verifier;

mod uses;

#[cfg(test)]
mod tests;

impl Verifier<'_> {
    pub(super) fn verify_storage_lifetimes(&mut self, function: MirDefinitionRef<'_>) {
        let entry_state = implicit_entry_storage(function);
        let mut incoming = vec![None; function.body().blocks.len()];
        let mut pending = VecDeque::new();
        let mut reported_joins = HashSet::new();

        if let Some(slot) = incoming.get_mut(function.body().entry.index()) {
            *slot = Some(entry_state.clone());
            pending.push_back(function.body().entry);
        }

        while let Some(block_id) = pending.pop_front() {
            let Some(block) = function.block(block_id) else {
                continue;
            };
            let Some(mut live) = incoming[block_id.index()].clone() else {
                continue;
            };
            self.apply_storage_lifetimes(function, block, &mut live, &entry_state);
            for successor in block.terminator.iter().flat_map(MirTerminator::successors) {
                merge_state(
                    self,
                    function,
                    block.id,
                    successor,
                    &live,
                    &mut incoming,
                    &mut pending,
                    &mut reported_joins,
                );
            }
        }

        // Structural verification does not exempt disconnected blocks. Seed
        // each remaining component deterministically with callable-entry
        // storage; explicit lifetime operations must establish every other
        // storage before use.
        for block in &function.body().blocks {
            let Some(slot) = incoming.get_mut(block.id.index()) else {
                continue;
            };
            if slot.is_some() {
                continue;
            }
            *slot = Some(entry_state.clone());
            pending.push_back(block.id);
            while let Some(block_id) = pending.pop_front() {
                let Some(block) = function.block(block_id) else {
                    continue;
                };
                let Some(mut live) = incoming[block_id.index()].clone() else {
                    continue;
                };
                self.apply_storage_lifetimes(function, block, &mut live, &entry_state);
                for successor in block.terminator.iter().flat_map(MirTerminator::successors) {
                    merge_state(
                        self,
                        function,
                        block.id,
                        successor,
                        &live,
                        &mut incoming,
                        &mut pending,
                        &mut reported_joins,
                    );
                }
            }
        }
    }

    fn apply_storage_lifetimes(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &crate::mir::MirBasicBlock,
        live: &mut BTreeSet<StorageId>,
        entry_state: &BTreeSet<StorageId>,
    ) {
        for instruction in &block.instructions {
            match instruction {
                MirInstruction::StorageLive(operation) => {
                    if function.storage(operation.storage).is_none() {
                        self.block_error(
                            function.callable(),
                            block.id,
                            format!(
                                "storage-live references undeclared storage {}",
                                operation.storage
                            ),
                        );
                    } else if !live.insert(operation.storage) {
                        self.block_error(
                            function.callable(),
                            block.id,
                            format!("storage {} is already live", operation.storage),
                        );
                    }
                }
                MirInstruction::StorageDead(operation) => {
                    if function.storage(operation.storage).is_none() {
                        self.block_error(
                            function.callable(),
                            block.id,
                            format!(
                                "storage-dead references undeclared storage {}",
                                operation.storage
                            ),
                        );
                    } else if !live.remove(&operation.storage) {
                        self.block_error(
                            function.callable(),
                            block.id,
                            format!("storage {} is already dead", operation.storage),
                        );
                    }
                }
                instruction => {
                    let mut used = BTreeSet::new();
                    uses::visit_instruction_storage(instruction, &mut |storage| {
                        used.insert(storage);
                    });
                    self.require_live_storage(function, block.id, live, used);
                }
            }
        }

        if let Some(terminator) = &block.terminator {
            let mut used = BTreeSet::new();
            uses::visit_terminator_storage(terminator, &mut |storage| {
                used.insert(storage);
            });
            self.require_live_storage(function, block.id, live, used);

            if matches!(
                terminator,
                MirTerminator::Return { .. }
                    | MirTerminator::ReturnShared { .. }
                    | MirTerminator::ReturnOptionalShared { .. }
            ) {
                for storage in live.difference(entry_state) {
                    self.block_error(
                        function.callable(),
                        block.id,
                        format!("storage {storage} remains live on normal return"),
                    );
                }
            }
        }
    }

    fn require_live_storage(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: crate::mir::BlockId,
        live: &BTreeSet<StorageId>,
        used: BTreeSet<StorageId>,
    ) {
        for storage in used {
            if function.storage(storage).is_some() && !live.contains(&storage) {
                self.block_error(
                    function.callable(),
                    block,
                    format!("storage {storage} is used outside a live lifetime epoch"),
                );
            }
        }
    }
}

fn implicit_entry_storage(function: MirDefinitionRef<'_>) -> BTreeSet<StorageId> {
    function
        .storage_entries()
        .iter()
        .filter_map(|storage| {
            matches!(
                storage.kind,
                MirStorageKind::Return
                    | MirStorageKind::Receiver
                    | MirStorageKind::Parameter
                    | MirStorageKind::AliasParameter(_)
            )
            .then_some(storage.id)
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn merge_state(
    verifier: &mut Verifier<'_>,
    function: MirDefinitionRef<'_>,
    predecessor: crate::mir::BlockId,
    target: crate::mir::BlockId,
    state: &BTreeSet<StorageId>,
    incoming: &mut [Option<BTreeSet<StorageId>>],
    pending: &mut VecDeque<crate::mir::BlockId>,
    reported_joins: &mut HashSet<crate::mir::BlockId>,
) {
    let Some(slot) = incoming.get_mut(target.index()) else {
        return;
    };
    match slot {
        None => {
            *slot = Some(state.clone());
            pending.push_back(target);
        }
        Some(existing) if existing == state => {}
        Some(existing) => {
            if reported_joins.insert(target) {
                verifier.block_error(
                    function.callable(),
                    predecessor,
                    format!("storage lifetime state disagrees at control-flow join {target}"),
                );
            }
            let merged: BTreeSet<_> = existing.intersection(state).copied().collect();
            if *existing != merged {
                *existing = merged;
                pending.push_back(target);
            }
        }
    }
}
