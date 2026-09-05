//! Definite initialization for ordinary primitive scalar storage.

use std::collections::HashSet;

use crate::mir::{
    MirDefinitionRef, MirInstruction, MirPlace, MirPlaceBase, MirRvalueKind, MirStorageKind,
    StorageId,
};

use super::{context::Verifier, contract::MirVerificationContract, dataflow::ForwardDataflow};

impl Verifier<'_> {
    pub(in crate::mir::verify) fn verify_scalar_initialization(
        &mut self,
        function: MirDefinitionRef<'_>,
    ) {
        // Normalized path activations are the only primitive storage whose
        // initialization relation relies on path proof consumed by the
        // mandatory normalizer. Every other primitive role, including an
        // ordinary compiler-owned ScalarSpill, uses this dataflow in both
        // verifier stages.
        let contract = self.verification_contract();
        let entry = function
            .storage_entries()
            .iter()
            .filter(|storage| {
                storage.ty.is_primitive()
                    && matches!(
                        storage.kind,
                        MirStorageKind::Parameter
                            | MirStorageKind::AliasParameter(_)
                            | MirStorageKind::Receiver
                    )
            })
            .map(|storage| storage.id)
            .collect::<HashSet<_>>();
        let mut flow = ForwardDataflow::new(function.callable(), function.body().blocks.len());
        flow.seed(function.body().entry, entry.clone());
        let mut reported = HashSet::new();

        // Definite initialization is checked only along executable entry
        // paths. A CFG transformation may deliberately leave a disconnected
        // component for a later optional cleanup pass; seeding that component
        // with an empty local state would make its edge into an otherwise
        // reachable join look like an executable uninitialized path.
        while let Some((block_id, mut initialized)) = flow.pop() {
            let Some(block) = function.block(block_id) else {
                continue;
            };
            for instruction in &block.instructions {
                match instruction {
                    MirInstruction::StorageLive(live)
                        if is_definite_initialization_storage(function, live.storage, contract) =>
                    {
                        initialized.remove(&live.storage);
                    }
                    MirInstruction::StorageDead(dead) => {
                        initialized.remove(&dead.storage);
                    }
                    MirInstruction::Store(store) => {
                        if let Some(storage) =
                            exact_primitive_place(function, &store.destination, contract)
                        {
                            initialized.insert(storage);
                        }
                    }
                    MirInstruction::Assign(assignment) => {
                        if let MirRvalueKind::Load(place) = &assignment.rvalue.kind {
                            if let Some(storage) = exact_primitive_place(function, place, contract)
                            {
                                if !initialized.contains(&storage)
                                    && reported.insert((block.id, storage))
                                {
                                    self.block_error(
                                        function.callable(),
                                        block.id,
                                        format!(
                                            "primitive storage {storage} is loaded without initialization on every incoming path"
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
                if let crate::mir::MirTerminator::OptionalUnwrap {
                    destination,
                    success_target,
                    failure_target,
                    ..
                } = terminator
                {
                    let mut success = initialized.clone();
                    if is_definite_initialization_storage(function, *destination, contract) {
                        success.insert(*destination);
                    }
                    merge_initialized(&mut flow, *success_target, &success);
                    merge_initialized(&mut flow, *failure_target, &initialized);
                } else {
                    for successor in terminator.successors() {
                        merge_initialized(&mut flow, successor, &initialized);
                    }
                }
            }
        }
    }
}

fn merge_initialized(
    flow: &mut ForwardDataflow<HashSet<StorageId>>,
    target: crate::mir::BlockId,
    initialized: &HashSet<StorageId>,
) {
    flow.merge(target, initialized, |existing, incoming| {
        let old_len = existing.len();
        existing.retain(|storage| incoming.contains(storage));
        existing.len() != old_len
    });
}

fn is_definite_initialization_storage(
    function: MirDefinitionRef<'_>,
    storage: StorageId,
    contract: MirVerificationContract,
) -> bool {
    function.storage(storage).is_some_and(|storage| {
        storage.ty.is_primitive() && !contract.trusts_consumed_path_initialization(storage.kind)
    })
}

fn exact_primitive_place(
    function: MirDefinitionRef<'_>,
    place: &MirPlace,
    contract: MirVerificationContract,
) -> Option<StorageId> {
    let MirPlaceBase::Storage(storage) = place.base else {
        return None;
    };
    (place.projections.is_empty()
        && is_definite_initialization_storage(function, storage, contract))
    .then_some(storage)
}
