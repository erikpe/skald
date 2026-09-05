//! Supported source snapshot, request, and result vocabulary for rehoming.

use std::collections::{BTreeMap, BTreeSet};

use crate::identity::CallableId;
use crate::mir::{
    BlockId, MirBasicBlock, MirDefinitionRef, MirLogicalExpression, MirPathCondition, MirStorage,
    MirStorageKind, MirValue, OptionalGuardId, PathConditionId, StorageId, ValueId,
};

use super::super::{
    edit::{collect_optional_guards, BlockPlacement, LogicalRecordIndex},
    error::MirRewriteError,
    identity::MirLocalId,
};

/// Owned immutable source snapshot for a cross-callable import.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MirImportSource {
    pub(super) callable: CallableId,
    pub(super) receiver: Option<StorageId>,
    pub(super) parameters: Vec<StorageId>,
    pub(super) return_storage: Option<StorageId>,
    pub(super) storage: Vec<MirStorage>,
    pub(super) values: Vec<MirValue>,
    pub(super) entry: BlockId,
    pub(super) blocks: Vec<MirBasicBlock>,
    pub(super) path_conditions: Vec<MirPathCondition>,
    pub(super) logical_expressions: Vec<MirLogicalExpression>,
    pub(super) optional_guards: BTreeSet<OptionalGuardId>,
}

impl MirImportSource {
    /// Takes an owned common-state snapshot without retaining program borrows.
    pub(crate) fn snapshot(definition: MirDefinitionRef<'_>) -> Result<Self, MirRewriteError> {
        let callable = definition.callable();
        let storage = definition.storage_entries().to_vec();
        validate_source_bindings(callable, &storage)?;
        let mut body = definition.body().clone();
        let optional_guards = collect_optional_guards(callable, &mut body)?;
        Ok(Self {
            callable,
            receiver: definition.receiver(),
            parameters: definition.parameters().to_vec(),
            return_storage: definition.return_storage(),
            storage,
            values: definition.values().to_vec(),
            entry: body.entry,
            blocks: body.blocks,
            path_conditions: body.path_conditions,
            logical_expressions: body.logical_expressions,
            optional_guards,
        })
    }

    pub(crate) const fn callable(&self) -> CallableId {
        self.callable
    }

    pub(crate) const fn receiver(&self) -> Option<StorageId> {
        self.receiver
    }

    pub(crate) fn parameters(&self) -> &[StorageId] {
        &self.parameters
    }

    pub(crate) const fn return_storage(&self) -> Option<StorageId> {
        self.return_storage
    }

    pub(crate) const fn entry(&self) -> BlockId {
        self.entry
    }

    pub(crate) fn storage_ids(&self) -> impl Iterator<Item = StorageId> + '_ {
        self.storage.iter().map(|storage| storage.id)
    }

    pub(crate) fn value_ids(&self) -> impl Iterator<Item = ValueId> + '_ {
        self.values.iter().map(|value| value.id)
    }

    pub(crate) fn block_ids(&self) -> impl Iterator<Item = BlockId> + '_ {
        self.blocks.iter().map(|block| block.id)
    }

    pub(crate) fn path_condition_ids(&self) -> impl Iterator<Item = PathConditionId> + '_ {
        self.path_conditions.iter().map(|condition| condition.id)
    }

    pub(crate) fn logical_record_indices(&self) -> impl Iterator<Item = usize> {
        0..self.logical_expressions.len()
    }

    pub(crate) fn optional_guard_ids(&self) -> impl Iterator<Item = OptionalGuardId> + '_ {
        self.optional_guards.iter().copied()
    }

    pub(super) fn storage(&self, identity: StorageId) -> Result<&MirStorage, MirRewriteError> {
        source_entry(self.callable, identity, &self.storage, |entry| entry.id)
    }

    pub(super) fn value(&self, identity: ValueId) -> Result<&MirValue, MirRewriteError> {
        source_entry(self.callable, identity, &self.values, |entry| entry.id)
    }

    pub(super) fn block(&self, identity: BlockId) -> Result<&MirBasicBlock, MirRewriteError> {
        source_entry(self.callable, identity, &self.blocks, |entry| entry.id)
    }

    pub(super) fn path_condition(
        &self,
        identity: PathConditionId,
    ) -> Result<&MirPathCondition, MirRewriteError> {
        source_entry(self.callable, identity, &self.path_conditions, |entry| {
            entry.id
        })
    }

    pub(super) fn optional_guard(&self, identity: OptionalGuardId) -> Result<(), MirRewriteError> {
        validate_source_owner(self.callable, identity)?;
        if self.optional_guards.contains(&identity) {
            Ok(())
        } else {
            Err(MirRewriteError::UnknownIdentity {
                identity: identity.local_identity(),
            })
        }
    }

    pub(super) fn logical_record(
        &self,
        index: usize,
    ) -> Result<&MirLogicalExpression, MirRewriteError> {
        self.logical_expressions
            .get(index)
            .ok_or(MirRewriteError::UnknownImportLogicalRecord {
                source: self.callable,
                index,
            })
    }

    #[cfg(test)]
    pub(super) fn from_common_parts(
        callable: CallableId,
        storage: Vec<MirStorage>,
        values: Vec<MirValue>,
        mut body: crate::mir::MirBody,
    ) -> Result<Self, MirRewriteError> {
        validate_source_bindings(callable, &storage)?;
        let optional_guards = collect_optional_guards(callable, &mut body)?;
        Ok(Self {
            callable,
            receiver: None,
            parameters: Vec::new(),
            return_storage: None,
            storage,
            values,
            entry: body.entry,
            blocks: body.blocks,
            path_conditions: body.path_conditions,
            logical_expressions: body.logical_expressions,
            optional_guards,
        })
    }
}

/// Explicit selected nodes, boundary substitutions, and block placement for
/// one import.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MirImportRequest {
    pub(super) storage: Vec<(StorageId, MirStorageKind)>,
    pub(super) values: Vec<ValueId>,
    pub(super) blocks: Vec<BlockId>,
    pub(super) path_conditions: Vec<PathConditionId>,
    pub(super) logical_records: Vec<usize>,
    pub(super) optional_guards: Vec<OptionalGuardId>,
    pub(super) storage_substitutions: Vec<(StorageId, StorageId)>,
    pub(super) value_substitutions: Vec<(ValueId, ValueId)>,
    pub(super) block_substitutions: Vec<(BlockId, BlockId)>,
    pub(super) path_condition_substitutions: Vec<(PathConditionId, PathConditionId)>,
    pub(super) optional_guard_substitutions: Vec<(OptionalGuardId, OptionalGuardId)>,
    pub(super) block_placement: BlockPlacement,
}

impl MirImportRequest {
    pub(crate) const fn new(block_placement: BlockPlacement) -> Self {
        Self {
            storage: Vec::new(),
            values: Vec::new(),
            blocks: Vec::new(),
            path_conditions: Vec::new(),
            logical_records: Vec::new(),
            optional_guards: Vec::new(),
            storage_substitutions: Vec::new(),
            value_substitutions: Vec::new(),
            block_substitutions: Vec::new(),
            path_condition_substitutions: Vec::new(),
            optional_guard_substitutions: Vec::new(),
            block_placement,
        }
    }

    pub(crate) fn import_storage(&mut self, source: StorageId, kind: MirStorageKind) {
        self.storage.push((source, kind));
    }

    pub(crate) fn import_value(&mut self, source: ValueId) {
        self.values.push(source);
    }

    pub(crate) fn import_block(&mut self, source: BlockId) {
        self.blocks.push(source);
    }

    pub(crate) fn import_path_condition(&mut self, source: PathConditionId) {
        self.path_conditions.push(source);
    }

    pub(crate) fn import_logical_record(&mut self, source_index: usize) {
        self.logical_records.push(source_index);
    }

    pub(crate) fn import_optional_guard(&mut self, source: OptionalGuardId) {
        self.optional_guards.push(source);
    }

    pub(crate) fn substitute_storage(&mut self, source: StorageId, destination: StorageId) {
        self.storage_substitutions.push((source, destination));
    }

    pub(crate) fn substitute_value(&mut self, source: ValueId, destination: ValueId) {
        self.value_substitutions.push((source, destination));
    }

    pub(crate) fn substitute_block(&mut self, source: BlockId, destination: BlockId) {
        self.block_substitutions.push((source, destination));
    }

    pub(crate) fn substitute_path_condition(
        &mut self,
        source: PathConditionId,
        destination: PathConditionId,
    ) {
        self.path_condition_substitutions
            .push((source, destination));
    }

    pub(crate) fn substitute_optional_guard(
        &mut self,
        source: OptionalGuardId,
        destination: OptionalGuardId,
    ) {
        self.optional_guard_substitutions
            .push((source, destination));
    }
}

/// Complete source-to-destination mapping for one local identity family.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MirImportMap<I> {
    pub(super) source: CallableId,
    pub(super) destination: CallableId,
    pub(super) entries: BTreeMap<I, I>,
}

impl<I: MirLocalId> MirImportMap<I> {
    pub(crate) fn destination(&self, source: I) -> Result<I, MirRewriteError> {
        validate_source_owner(self.source, source)?;
        self.entries
            .get(&source)
            .copied()
            .ok_or(MirRewriteError::UnknownIdentity {
                identity: source.local_identity(),
            })
    }

    pub(crate) const fn destination_callable(&self) -> CallableId {
        self.destination
    }

    pub(super) fn empty(source: CallableId, destination: CallableId) -> Self {
        Self {
            source,
            destination,
            entries: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MirImportMaps {
    pub(crate) storage: MirImportMap<StorageId>,
    pub(crate) values: MirImportMap<ValueId>,
    pub(crate) blocks: MirImportMap<BlockId>,
    pub(crate) path_conditions: MirImportMap<PathConditionId>,
    pub(crate) optional_guards: MirImportMap<OptionalGuardId>,
}

impl MirImportMaps {
    pub(super) fn empty(source: CallableId, destination: CallableId) -> Self {
        Self {
            storage: MirImportMap::empty(source, destination),
            values: MirImportMap::empty(source, destination),
            blocks: MirImportMap::empty(source, destination),
            path_conditions: MirImportMap::empty(source, destination),
            optional_guards: MirImportMap::empty(source, destination),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MirImportResult {
    pub(crate) maps: MirImportMaps,
    pub(crate) logical_records: Vec<(usize, LogicalRecordIndex)>,
}

pub(super) fn validate_source_owner<I: MirLocalId>(
    expected: CallableId,
    identity: I,
) -> Result<(), MirRewriteError> {
    if identity.callable() == expected {
        Ok(())
    } else {
        Err(MirRewriteError::ForeignIdentity {
            expected,
            identity: identity.local_identity(),
        })
    }
}

pub(super) fn is_imported_storage_kind(kind: MirStorageKind) -> bool {
    match kind {
        MirStorageKind::Return
        | MirStorageKind::Receiver
        | MirStorageKind::Parameter
        | MirStorageKind::AliasParameter(_) => false,
        MirStorageKind::CheckedView(_)
        | MirStorageKind::Local
        | MirStorageKind::Argument
        | MirStorageKind::Temporary
        | MirStorageKind::SharedAnchor
        | MirStorageKind::ScalarSpill
        | MirStorageKind::PrimitiveAlias
        | MirStorageKind::PathCondition
        | MirStorageKind::NormalizedPathActivation
        | MirStorageKind::OptionalUnwrap
        | MirStorageKind::SharedAllocation
        | MirStorageKind::ArrayBacking
        | MirStorageKind::ArrayProduced
        | MirStorageKind::ArraySlice
        | MirStorageKind::ArrayPosition
        | MirStorageKind::ArrayAnchor(_)
        | MirStorageKind::ArrayAlias(_) => true,
    }
}

fn validate_source_bindings(
    expected: CallableId,
    storage: &[MirStorage],
) -> Result<(), MirRewriteError> {
    for declaration in storage {
        if let Some(binding) = declaration.source {
            if binding.callable() != expected {
                return Err(MirRewriteError::ForeignImportBinding {
                    expected,
                    storage: declaration.id,
                    binding,
                });
            }
        }
    }
    Ok(())
}

fn source_entry<I, T>(
    expected: CallableId,
    identity: I,
    entries: &[T],
    entry_id: impl Fn(&T) -> I,
) -> Result<&T, MirRewriteError>
where
    I: MirLocalId,
{
    validate_source_owner(expected, identity)?;
    entries
        .get(identity.index())
        .filter(|entry| entry_id(entry) == identity)
        .ok_or(MirRewriteError::UnknownIdentity {
            identity: identity.local_identity(),
        })
}
