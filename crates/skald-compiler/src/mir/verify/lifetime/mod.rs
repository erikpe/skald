//! Dynamic storage-lifetime epoch verification.

use std::collections::{BTreeSet, HashSet};

use crate::mir::{MirDefinitionRef, MirInstruction, MirStorageKind, MirTerminator, StorageId};

use super::{context::Verifier, dataflow::ForwardDataflow};

mod uses;

#[cfg(test)]
mod tests;

impl Verifier<'_> {
    pub(super) fn verify_storage_lifetimes(&mut self, function: MirDefinitionRef<'_>) {
        let entry_state = implicit_entry_storage(function);
        let mut flow = ForwardDataflow::new(function.callable(), function.body().blocks.len());
        let mut reported_joins = HashSet::new();

        flow.seed(function.body().entry, entry_state.clone());
        loop {
            while let Some((block_id, mut live)) = flow.pop() {
                let Some(block) = function.block(block_id) else {
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
                        &mut flow,
                        &mut reported_joins,
                    );
                }
            }
            if !flow.seed_next_component(&function.body().blocks, entry_state.clone()) {
                break;
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
    flow: &mut ForwardDataflow<BTreeSet<StorageId>>,
    reported_joins: &mut HashSet<crate::mir::BlockId>,
) {
    flow.merge(target, state, |existing, incoming| {
        if existing == incoming {
            return false;
        }
        if reported_joins.insert(target) {
            verifier.block_error(
                function.callable(),
                predecessor,
                format!("storage lifetime state disagrees at control-flow join {target}"),
            );
        }
        let merged: BTreeSet<_> = existing.intersection(incoming).copied().collect();
        if *existing != merged {
            *existing = merged;
            true
        } else {
            false
        }
    });
}
