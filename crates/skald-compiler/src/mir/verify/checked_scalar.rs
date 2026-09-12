//! Shared carrier queries for verified checked-scalar diamonds.

use super::super::model::{
    BlockId, MirDefinitionRef, MirInstruction, MirPlace, MirRvalueKind, StorageId,
};

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

pub(super) fn is_exact_load(rvalue: &MirRvalueKind, storage: StorageId) -> bool {
    matches!(rvalue, MirRvalueKind::Load(place) if *place == MirPlace::base(storage))
}
