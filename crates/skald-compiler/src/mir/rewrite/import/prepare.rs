//! Deterministic validation and normalization of an import request.

use std::collections::BTreeMap;

use crate::identity::CallableId;
use crate::mir::{BlockId, MirStorageKind, OptionalGuardId, PathConditionId, StorageId, ValueId};

use super::super::{
    edit::{BlockPlacement, MirCallableEdit},
    error::MirRewriteError,
    identity::MirLocalId,
    MirLocalIdentity, MirLocalIdentitySite,
};
use super::model::{
    is_imported_storage_kind, validate_source_owner, MirImportMaps, MirImportRequest,
    MirImportSource,
};

pub(super) struct PreparedRequest {
    pub(super) storage: Vec<(StorageId, MirStorageKind)>,
    pub(super) values: Vec<ValueId>,
    pub(super) blocks: Vec<BlockId>,
    pub(super) path_conditions: Vec<PathConditionId>,
    pub(super) logical_records: Vec<usize>,
    pub(super) optional_guards: Vec<OptionalGuardId>,
    storage_substitutions: BTreeMap<StorageId, StorageId>,
    value_substitutions: BTreeMap<ValueId, ValueId>,
    block_substitutions: BTreeMap<BlockId, BlockId>,
    path_condition_substitutions: BTreeMap<PathConditionId, PathConditionId>,
    optional_guard_substitutions: BTreeMap<OptionalGuardId, OptionalGuardId>,
    pub(super) block_placement: BlockPlacement,
}

impl PreparedRequest {
    pub(super) fn new(
        source: &MirImportSource,
        destination: &MirCallableEdit,
        request: MirImportRequest,
    ) -> Result<Self, MirRewriteError> {
        let storage = prepare_storage(source, request.storage)?;
        let values = prepare_selection(source.callable, request.values, |id| {
            source.value(id).map(|_| ())
        })?;
        let blocks = prepare_selection(source.callable, request.blocks, |id| {
            source.block(id).map(|_| ())
        })?;
        let path_conditions = prepare_selection(source.callable, request.path_conditions, |id| {
            source.path_condition(id).map(|_| ())
        })?;
        let optional_guards = prepare_selection(source.callable, request.optional_guards, |id| {
            source.optional_guard(id)
        })?;
        let logical_records = prepare_logical_records(source, request.logical_records)?;
        let storage_substitutions = prepare_substitutions(
            source.callable,
            request.storage_substitutions,
            |source_id, destination_id| {
                let source_storage = source.storage(source_id)?;
                let destination_storage = destination.storage(destination_id)?;
                if source_storage.ty == destination_storage.ty {
                    Ok(())
                } else {
                    Err(MirRewriteError::StorageTypeMismatch {
                        from: source_id,
                        from_type: source_storage.ty,
                        to: destination_id,
                        to_type: destination_storage.ty,
                    })
                }
            },
        )?;
        let value_substitutions = prepare_substitutions(
            source.callable,
            request.value_substitutions,
            |source_id, destination_id| {
                let source_value = source.value(source_id)?;
                let destination_value = destination.value(destination_id)?;
                if source_value.ty == destination_value.ty {
                    Ok(())
                } else {
                    Err(MirRewriteError::ValueTypeMismatch {
                        from: source_id,
                        from_type: source_value.ty,
                        to: destination_id,
                        to_type: destination_value.ty,
                    })
                }
            },
        )?;
        let block_substitutions = prepare_substitutions(
            source.callable,
            request.block_substitutions,
            |source_id, destination_id| {
                source.block(source_id)?;
                destination.block(destination_id)?;
                Ok(())
            },
        )?;
        let path_condition_substitutions = prepare_substitutions(
            source.callable,
            request.path_condition_substitutions,
            |source_id, destination_id| {
                source.path_condition(source_id)?;
                destination.path_condition(destination_id)?;
                Ok(())
            },
        )?;
        let optional_guard_substitutions = prepare_substitutions(
            source.callable,
            request.optional_guard_substitutions,
            |source_id, destination_id| {
                source.optional_guard(source_id)?;
                destination.optional_guard(destination_id)?;
                Ok(())
            },
        )?;

        reject_selected_substitutions(
            &storage.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
            &storage_substitutions,
        )?;
        reject_selected_substitutions(&values, &value_substitutions)?;
        reject_selected_substitutions(&blocks, &block_substitutions)?;
        reject_selected_substitutions(&path_conditions, &path_condition_substitutions)?;
        reject_selected_substitutions(&optional_guards, &optional_guard_substitutions)?;
        require_callable_attachment_substitutions(source, &storage_substitutions)?;

        Ok(Self {
            storage,
            values,
            blocks,
            path_conditions,
            logical_records,
            optional_guards,
            storage_substitutions,
            value_substitutions,
            block_substitutions,
            path_condition_substitutions,
            optional_guard_substitutions,
            block_placement: request.block_placement,
        })
    }

    pub(super) fn substitution_maps(
        &self,
        source: CallableId,
        destination: CallableId,
    ) -> MirImportMaps {
        let mut maps = MirImportMaps::empty(source, destination);
        maps.storage.entries = self.storage_substitutions.clone();
        maps.values.entries = self.value_substitutions.clone();
        maps.blocks.entries = self.block_substitutions.clone();
        maps.path_conditions.entries = self.path_condition_substitutions.clone();
        maps.optional_guards.entries = self.optional_guard_substitutions.clone();
        maps
    }
}

fn prepare_storage(
    source: &MirImportSource,
    mut selection: Vec<(StorageId, MirStorageKind)>,
) -> Result<Vec<(StorageId, MirStorageKind)>, MirRewriteError> {
    selection.sort_by_key(|(identity, _)| *identity);
    let mut previous = None;
    for (identity, kind) in selection.iter().copied() {
        source.storage(identity)?;
        if previous == Some(identity) {
            return Err(MirRewriteError::DuplicateImportIdentity {
                identity: identity.local_identity(),
            });
        }
        if !is_imported_storage_kind(kind) {
            return Err(MirRewriteError::InvalidImportStorageKind {
                storage: identity,
                kind,
            });
        }
        previous = Some(identity);
    }
    Ok(selection)
}

fn prepare_selection<I: MirLocalId>(
    source: CallableId,
    mut selection: Vec<I>,
    mut validate: impl FnMut(I) -> Result<(), MirRewriteError>,
) -> Result<Vec<I>, MirRewriteError> {
    selection.sort();
    let mut previous = None;
    for identity in selection.iter().copied() {
        validate_source_owner(source, identity)?;
        validate(identity)?;
        if previous == Some(identity) {
            return Err(MirRewriteError::DuplicateImportIdentity {
                identity: identity.local_identity(),
            });
        }
        previous = Some(identity);
    }
    Ok(selection)
}

fn prepare_substitutions<I: MirLocalId>(
    source: CallableId,
    substitutions: Vec<(I, I)>,
    mut validate: impl FnMut(I, I) -> Result<(), MirRewriteError>,
) -> Result<BTreeMap<I, I>, MirRewriteError> {
    let mut prepared = BTreeMap::new();
    for (source_id, destination_id) in substitutions {
        validate_source_owner(source, source_id)?;
        validate(source_id, destination_id)?;
        if prepared.insert(source_id, destination_id).is_some() {
            return Err(MirRewriteError::DuplicateImportSubstitution {
                identity: source_id.local_identity(),
            });
        }
    }
    Ok(prepared)
}

fn prepare_logical_records(
    source: &MirImportSource,
    mut records: Vec<usize>,
) -> Result<Vec<usize>, MirRewriteError> {
    records.sort_unstable();
    let mut previous = None;
    for index in records.iter().copied() {
        source.logical_record(index)?;
        if previous == Some(index) {
            return Err(MirRewriteError::DuplicateImportLogicalRecord {
                source: source.callable,
                index,
            });
        }
        previous = Some(index);
    }
    Ok(records)
}

fn reject_selected_substitutions<I: MirLocalId>(
    selected: &[I],
    substitutions: &BTreeMap<I, I>,
) -> Result<(), MirRewriteError> {
    if let Some(identity) = selected
        .iter()
        .copied()
        .find(|identity| substitutions.contains_key(identity))
    {
        Err(MirRewriteError::SelectedImportIdentityHasSubstitution {
            identity: identity.local_identity(),
        })
    } else {
        Ok(())
    }
}

fn require_callable_attachment_substitutions(
    source: &MirImportSource,
    substitutions: &BTreeMap<StorageId, StorageId>,
) -> Result<(), MirRewriteError> {
    if let Some(receiver) = source.receiver {
        require_storage_substitution(substitutions, receiver, MirLocalIdentitySite::Receiver)?;
    }
    for (index, parameter) in source.parameters.iter().copied().enumerate() {
        require_storage_substitution(
            substitutions,
            parameter,
            MirLocalIdentitySite::Parameter(index),
        )?;
    }
    if let Some(return_storage) = source.return_storage {
        require_storage_substitution(
            substitutions,
            return_storage,
            MirLocalIdentitySite::ReturnStorage,
        )?;
    }
    Ok(())
}

fn require_storage_substitution(
    substitutions: &BTreeMap<StorageId, StorageId>,
    identity: StorageId,
    site: MirLocalIdentitySite,
) -> Result<(), MirRewriteError> {
    if substitutions.contains_key(&identity) {
        Ok(())
    } else {
        Err(MirRewriteError::MissingImportSubstitution {
            identity: MirLocalIdentity::Storage(identity),
            site,
        })
    }
}
