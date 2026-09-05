//! Definite initialization for non-null function-value storage.

use std::collections::HashSet;

use crate::mir::{
    MirDefinitionRef, MirInstruction, MirPlace, MirPlaceBase, MirRvalueKind, MirStorageKind,
    MirType, StorageId,
};

use super::super::{context::Verifier, dataflow::ForwardDataflow};

impl Verifier<'_> {
    pub(in crate::mir::verify) fn verify_function_value_provenance(
        &mut self,
        function: MirDefinitionRef<'_>,
    ) {
        let entry = function
            .storage_entries()
            .iter()
            .filter(|storage| {
                storage.kind == MirStorageKind::Parameter
                    && matches!(storage.ty, MirType::Function(_))
            })
            .map(|storage| storage.id)
            .collect::<HashSet<_>>();
        let mut flow = ForwardDataflow::new(function.callable(), function.body().blocks.len());
        flow.seed(function.body().entry, entry.clone());
        let mut reported = HashSet::new();

        // Definite initialization is an executable-path property. Optimization
        // may leave disconnected blocks for the independently selectable CFG
        // cleanup pass; seeding those components with an empty local state can
        // otherwise contaminate a reachable join they still target.
        while let Some((block_id, mut initialized)) = flow.pop() {
            let Some(block) = function.block(block_id) else {
                continue;
            };
            for instruction in &block.instructions {
                match instruction {
                    MirInstruction::StorageLive(live)
                        if is_local_function_storage(function, live.storage) =>
                    {
                        initialized.remove(&live.storage);
                    }
                    MirInstruction::StorageDead(dead) => {
                        initialized.remove(&dead.storage);
                    }
                    MirInstruction::Store(store) => {
                        if let Some(storage) =
                            exact_local_function_place(function, &store.destination)
                        {
                            initialized.insert(storage);
                        }
                    }
                    MirInstruction::Assign(assignment) => {
                        if let MirRvalueKind::Load(place) = &assignment.rvalue.kind {
                            if let Some(storage) = exact_local_function_place(function, place) {
                                if !initialized.contains(&storage)
                                    && reported.insert((block.id, storage))
                                {
                                    self.block_error(
                                        function.callable(),
                                        block.id,
                                        format!(
                                            "function-value storage {storage} is loaded without non-null initialization on every incoming path"
                                        ),
                                    );
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            if let Some(terminator) = &block.terminator {
                for successor in terminator.successors() {
                    flow.merge(successor, &initialized, |existing, incoming| {
                        let old_len = existing.len();
                        existing.retain(|storage| incoming.contains(storage));
                        existing.len() != old_len
                    });
                }
            }
        }
    }
}

fn is_local_function_storage(function: MirDefinitionRef<'_>, storage: StorageId) -> bool {
    function
        .storage(storage)
        .is_some_and(|storage| matches!(storage.ty, MirType::Function(_)))
}

fn exact_local_function_place(
    function: MirDefinitionRef<'_>,
    place: &MirPlace,
) -> Option<StorageId> {
    let MirPlaceBase::Storage(storage) = place.base else {
        return None;
    };
    (place.projections.is_empty() && is_local_function_storage(function, storage))
        .then_some(storage)
}
