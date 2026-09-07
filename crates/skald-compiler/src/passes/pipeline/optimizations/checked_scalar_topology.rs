//! Shared structural vocabulary for checked scalar protocols.
//!
//! Protocol-specific topology remains in its owning module. This module owns
//! only exact instruction/value sites and low-level shape checks that are
//! identical for checked integer operations and checked floating casts.

use std::collections::{HashMap, HashSet};

use crate::mir::{
    rewrite::{
        MirCallableEdit, MirLocalCfgFacts, MirLocalIdentity, MirLocalIdentitySite,
        MirReferenceFailure, MirRewriteError,
    },
    BlockId, MirBasicBlock, MirDefinitionRef, MirInstruction, MirPlace, MirRvalueKind, MirStorage,
    MirType, StorageId, ValueId,
};
use crate::source::Span;

/// One instruction's stable location in the current dense callable snapshot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct CheckedScalarInstructionSite {
    pub(super) block: BlockId,
    pub(super) instruction: usize,
}

/// One value definition and its exact location and source span.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct CheckedScalarValueSite {
    pub(super) value: ValueId,
    pub(super) site: CheckedScalarInstructionSite,
    pub(super) span: Span,
}

pub(super) fn storage_write_sites(
    definition: MirDefinitionRef<'_>,
    storage: StorageId,
) -> Vec<CheckedScalarInstructionSite> {
    definition
        .body()
        .blocks
        .iter()
        .flat_map(|block| {
            block
                .instructions
                .iter()
                .enumerate()
                .filter_map(move |(instruction, value)| {
                    matches!(
                        value,
                        MirInstruction::Store(store)
                            if store.destination == MirPlace::base(storage)
                    )
                    .then_some(CheckedScalarInstructionSite {
                        block: block.id,
                        instruction,
                    })
                })
        })
        .collect()
}

pub(super) fn edit_storage_write_sites(
    edit: &MirCallableEdit,
    storage: StorageId,
) -> Vec<CheckedScalarInstructionSite> {
    edit.block_order()
        .iter()
        .flat_map(|block| {
            edit.block(*block)
                .expect("block order contains only live blocks")
                .instructions
                .iter()
                .enumerate()
                .filter_map(move |(instruction, value)| {
                    matches!(
                        value,
                        MirInstruction::Store(store)
                            if store.destination == MirPlace::base(storage)
                    )
                    .then_some(CheckedScalarInstructionSite {
                        block: *block,
                        instruction,
                    })
                })
        })
        .collect()
}

pub(super) fn cfg_predecessors(cfg: &MirLocalCfgFacts) -> HashMap<BlockId, HashSet<BlockId>> {
    let mut predecessors = HashMap::<_, HashSet<_>>::new();
    for block in cfg.blocks() {
        for successor in block.successors() {
            predecessors
                .entry(*successor)
                .or_default()
                .insert(block.block());
        }
    }
    predecessors
}

pub(super) fn exact_first_load(
    block: &MirBasicBlock,
    storage: StorageId,
    ty: MirType,
) -> Option<CheckedScalarValueSite> {
    let Some(MirInstruction::Assign(load)) = block.instructions.first() else {
        return None;
    };
    (is_exact_load(&load.rvalue.kind, storage) && load.rvalue.ty == ty).then_some(
        CheckedScalarValueSite {
            value: load.result,
            site: CheckedScalarInstructionSite {
                block: block.id,
                instruction: 0,
            },
            span: load.span,
        },
    )
}

pub(super) fn is_exact_load(kind: &MirRvalueKind, storage: StorageId) -> bool {
    matches!(kind, MirRvalueKind::Load(place) if *place == MirPlace::base(storage))
}

pub(super) fn has_only_predecessor(
    predecessors: &HashMap<BlockId, HashSet<BlockId>>,
    block: BlockId,
    expected: BlockId,
) -> bool {
    predecessors.get(&block) == Some(&HashSet::from([expected]))
}

pub(super) fn required_storage(
    definition: MirDefinitionRef<'_>,
    storage: StorageId,
    check_block: BlockId,
) -> Result<&MirStorage, MirRewriteError> {
    definition.storage(storage).ok_or_else(|| {
        invalid_reference(
            definition,
            MirLocalIdentity::Storage(storage),
            MirLocalIdentitySite::Terminator(check_block.index()),
        )
    })
}

pub(super) fn invalid_block(
    definition: MirDefinitionRef<'_>,
    block: BlockId,
    referencing_block: BlockId,
) -> MirRewriteError {
    invalid_reference(
        definition,
        MirLocalIdentity::Block(block),
        MirLocalIdentitySite::Terminator(referencing_block.index()),
    )
}

fn invalid_reference(
    definition: MirDefinitionRef<'_>,
    identity: MirLocalIdentity,
    site: MirLocalIdentitySite,
) -> MirRewriteError {
    let failure = if identity.callable() == definition.callable() {
        MirReferenceFailure::Unknown
    } else {
        MirReferenceFailure::Foreign
    };
    MirRewriteError::InvalidReference {
        expected: definition.callable(),
        identity,
        site,
        failure,
    }
}
