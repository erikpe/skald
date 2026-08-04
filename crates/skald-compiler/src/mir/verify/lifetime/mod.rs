//! Dynamic storage-lifetime epoch verification.

use std::collections::{BTreeSet, HashMap, HashSet};

use crate::mir::{MirDefinitionRef, MirInstruction, MirStorageKind, MirTerminator, StorageId};

use super::{
    context::Verifier,
    dataflow::ForwardDataflow,
    path_state::{condition_reads, PathStates},
};

mod uses;

#[cfg(test)]
mod tests;

impl Verifier<'_> {
    pub(super) fn verify_storage_lifetimes(&mut self, function: MirDefinitionRef<'_>) {
        self.verify_temporary_lifetime_shape(function);
        let entry_state = implicit_entry_storage(function);
        let condition_reads = condition_reads(function);
        let activation_conditions: HashMap<_, _> = function
            .path_conditions()
            .iter()
            .map(|condition| (condition.activation, condition.id))
            .collect();
        let mut flow = ForwardDataflow::new(function.callable(), function.body().blocks.len());
        let mut reported_joins = HashSet::new();
        let mut reported_condition_ends = HashSet::new();

        flow.seed(
            function.body().entry,
            PathStates::initial(entry_state.clone()),
        );
        loop {
            while let Some((block_id, mut states)) = flow.pop() {
                let Some(block) = function.block(block_id) else {
                    continue;
                };
                states.update_states(|live| {
                    self.apply_storage_lifetimes(function, block, live, &entry_state);
                });
                for instruction in &block.instructions {
                    let MirInstruction::StorageDead(operation) = instruction else {
                        continue;
                    };
                    let Some(condition) = activation_conditions.get(&operation.storage).copied()
                    else {
                        continue;
                    };
                    for child in function
                        .path_conditions()
                        .iter()
                        .filter(|candidate| candidate.parent == Some(condition))
                    {
                        if states.any_select(child.id)
                            && reported_condition_ends.insert((block.id, condition))
                        {
                            self.block_error(
                                function.callable(),
                                block.id,
                                format!(
                                    "path condition {condition} ends while child {} remains selected",
                                    child.id
                                ),
                            );
                        }
                    }
                    let missing = states.end_condition(condition, |existing, incoming| {
                        if reported_condition_ends.insert((block.id, condition)) {
                            self.block_error(
                                function.callable(),
                                block.id,
                                format!(
                                    "conditional storage lifetime state remains when path condition {condition} ends"
                                ),
                            );
                        }
                        *existing = existing.intersection(incoming).copied().collect();
                    });
                    if missing && reported_condition_ends.insert((block.id, condition)) {
                        self.block_error(
                            function.callable(),
                            block.id,
                            format!(
                                "path condition {condition} ends outside its selected control-flow region"
                            ),
                        );
                    }
                }
                let Some(terminator) = &block.terminator else {
                    continue;
                };
                if let MirTerminator::Branch {
                    condition,
                    true_target,
                    false_target,
                    ..
                } = terminator
                {
                    if let Some(path_condition) = condition_reads.get(condition).copied() {
                        for (successor, active) in [(*true_target, true), (*false_target, false)] {
                            let (selected, _) = states.select(path_condition, active);
                            merge_state(
                                self,
                                function,
                                block.id,
                                successor,
                                &selected,
                                &mut flow,
                                &mut reported_joins,
                            );
                        }
                        continue;
                    }
                }
                for successor in terminator.successors() {
                    merge_state(
                        self,
                        function,
                        block.id,
                        successor,
                        &states,
                        &mut flow,
                        &mut reported_joins,
                    );
                }
            }
            if !flow.seed_next_component(
                &function.body().blocks,
                PathStates::initial(entry_state.clone()),
            ) {
                break;
            }
        }
    }

    /// Hidden owning temporaries represent one expression evaluation site.
    /// Reusing their static storage identity for another epoch would make
    /// completion order and full-expression cleanup ambiguous.
    fn verify_temporary_lifetime_shape(&mut self, function: MirDefinitionRef<'_>) {
        for storage in function
            .storage_entries()
            .iter()
            .filter(|storage| storage.kind == MirStorageKind::Temporary)
        {
            let mut starts = 0;
            let mut ends = 0;
            for instruction in function
                .body()
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
            {
                match instruction {
                    MirInstruction::StorageLive(operation) if operation.storage == storage.id => {
                        starts += 1;
                    }
                    MirInstruction::StorageDead(operation) if operation.storage == storage.id => {
                        ends += 1;
                    }
                    _ => {}
                }
            }
            if starts != 1 || ends > 1 {
                self.block_error(
                    function.callable(),
                    function.body().entry,
                    format!(
                        "temporary storage {} must have one non-reused lifetime epoch, found {starts} starts and {ends} ends",
                        storage.id
                    ),
                );
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
    state: &PathStates<BTreeSet<StorageId>>,
    flow: &mut ForwardDataflow<PathStates<BTreeSet<StorageId>>>,
    reported_joins: &mut HashSet<crate::mir::BlockId>,
) {
    if state.is_empty() {
        return;
    }
    let selected = state
        .on_edge(function, predecessor, target)
        .unwrap_or_else(|_| state.clone());
    flow.merge(target, &selected, |existing, incoming| {
        existing.merge(incoming, |existing, incoming| {
            if reported_joins.insert(target) {
                verifier.block_error(
                    function.callable(),
                    predecessor,
                    format!("storage lifetime state disagrees at control-flow join {target}"),
                );
            }
            *existing = existing.intersection(incoming).copied().collect();
        })
    });
}
