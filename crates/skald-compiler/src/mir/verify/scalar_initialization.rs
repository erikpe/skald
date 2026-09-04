//! Definite initialization for ordinary primitive scalar storage.

use std::collections::HashSet;

use crate::mir::{
    MirDefinitionRef, MirInstruction, MirPlace, MirPlaceBase, MirRvalueKind, MirStorageKind,
    StorageId,
};

use super::{context::Verifier, dataflow::ForwardDataflow};

impl Verifier<'_> {
    pub(in crate::mir::verify) fn verify_scalar_initialization(
        &mut self,
        function: MirDefinitionRef<'_>,
    ) {
        // Compiler-owned scalar spills are checked while their producing
        // protocol evidence is present. Former path activations intentionally
        // become indistinguishable ScalarSpill storage after normalization,
        // so the normalized contract relies on the consumed-proof authority
        // and continues checking every source-visible primitive storage kind.
        let verify_scalar_spills = self.verification_contract().requires_proof_provenance();
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

        loop {
            while let Some((block_id, mut initialized)) = flow.pop() {
                let Some(block) = function.block(block_id) else {
                    continue;
                };
                for instruction in &block.instructions {
                    match instruction {
                        MirInstruction::StorageLive(live)
                            if is_definite_initialization_storage(
                                function,
                                live.storage,
                                verify_scalar_spills,
                            ) =>
                        {
                            initialized.remove(&live.storage);
                        }
                        MirInstruction::StorageDead(dead) => {
                            initialized.remove(&dead.storage);
                        }
                        MirInstruction::Store(store) => {
                            if let Some(storage) = exact_primitive_place(
                                function,
                                &store.destination,
                                verify_scalar_spills,
                            ) {
                                initialized.insert(storage);
                            }
                        }
                        MirInstruction::Assign(assignment) => {
                            if let MirRvalueKind::Load(place) = &assignment.rvalue.kind {
                                if let Some(storage) =
                                    exact_primitive_place(function, place, verify_scalar_spills)
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
                        if is_definite_initialization_storage(
                            function,
                            *destination,
                            verify_scalar_spills,
                        ) {
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
            if !flow.seed_next_component(&function.body().blocks, entry.clone()) {
                break;
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
    verify_scalar_spills: bool,
) -> bool {
    function.storage(storage).is_some_and(|storage| {
        storage.ty.is_primitive()
            && (verify_scalar_spills || storage.kind != MirStorageKind::ScalarSpill)
    })
}

fn exact_primitive_place(
    function: MirDefinitionRef<'_>,
    place: &MirPlace,
    verify_scalar_spills: bool,
) -> Option<StorageId> {
    let MirPlaceBase::Storage(storage) = place.base else {
        return None;
    };
    (place.projections.is_empty()
        && is_definite_initialization_storage(function, storage, verify_scalar_spills))
    .then_some(storage)
}
