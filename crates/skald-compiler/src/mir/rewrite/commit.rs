//! Atomic compaction of one sparse common callable transaction.

use std::collections::BTreeSet;

use crate::identity::CallableId;

use super::super::{
    BlockId, MirBody, MirStorage, MirValue, OptionalGuardId, PathConditionId, StorageId, ValueId,
};
use super::{
    edit::{EditIdentityInventory, EditRecordInventory, MirCallableEdit, MirCallableEditInventory},
    error::{MirReferenceFailure, MirRewriteError},
    identity::MirLocalId,
    map::map_common_local_identities,
    MirLocalIdentityMapper, MirLocalIdentitySite,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SourceSlot<I> {
    Unallocated,
    Deleted,
    Retained(I),
}

/// Complete typed mapping from transaction slots to committed dense IDs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MirCommitMap<I> {
    owner: CallableId,
    slots: Vec<SourceSlot<I>>,
}

impl<I: MirLocalId> MirCommitMap<I> {
    pub(crate) fn committed(&self, identity: I) -> Result<I, MirRewriteError> {
        if identity.callable() != self.owner {
            return Err(MirRewriteError::ForeignIdentity {
                expected: self.owner,
                identity: identity.local_identity(),
            });
        }
        match self.slots.get(identity.index()) {
            Some(SourceSlot::Retained(committed)) => Ok(*committed),
            Some(SourceSlot::Deleted) => Err(MirRewriteError::DeletedIdentity {
                identity: identity.local_identity(),
            }),
            Some(SourceSlot::Unallocated) | None => Err(MirRewriteError::UnknownIdentity {
                identity: identity.local_identity(),
            }),
        }
    }

    fn reference(&self, identity: I, site: MirLocalIdentitySite) -> Result<I, MirRewriteError> {
        let failure = if identity.callable() != self.owner {
            MirReferenceFailure::Foreign
        } else {
            match self.slots.get(identity.index()) {
                Some(SourceSlot::Retained(committed)) => return Ok(*committed),
                Some(SourceSlot::Deleted) => MirReferenceFailure::Deleted,
                Some(SourceSlot::Unallocated) | None => MirReferenceFailure::Unknown,
            }
        };
        Err(MirRewriteError::InvalidReference {
            expected: self.owner,
            identity: identity.local_identity(),
            site,
            failure,
        })
    }

    fn build_with_owner(
        owner: CallableId,
        inventory: &EditIdentityInventory<I>,
    ) -> Result<Self, MirRewriteError> {
        let mut slots = inventory
            .slots
            .iter()
            .map(|state| match state {
                None => SourceSlot::Unallocated,
                Some(false) => SourceSlot::Deleted,
                Some(true) => SourceSlot::Unallocated,
            })
            .collect::<Vec<_>>();
        let mut seen = BTreeSet::new();
        for (dense_index, source) in inventory.order.iter().copied().enumerate() {
            if source.callable() != owner {
                return Err(MirRewriteError::ForeignIdentity {
                    expected: owner,
                    identity: source.local_identity(),
                });
            }
            if !seen.insert(source) {
                return Err(MirRewriteError::DuplicateOrderIdentity {
                    identity: source.local_identity(),
                });
            }
            match inventory.slots.get(source.index()) {
                Some(Some(true)) => {
                    slots[source.index()] = SourceSlot::Retained(I::new(owner, dense_index));
                }
                Some(Some(false)) => {
                    return Err(MirRewriteError::DeletedIdentity {
                        identity: source.local_identity(),
                    });
                }
                Some(None) | None => {
                    return Err(MirRewriteError::UnknownIdentity {
                        identity: source.local_identity(),
                    });
                }
            }
        }
        for (index, state) in inventory.slots.iter().enumerate() {
            if *state == Some(true) && !seen.contains(&I::new(owner, index)) {
                return Err(MirRewriteError::MissingOrderIdentity {
                    identity: I::new(owner, index).local_identity(),
                });
            }
        }
        Ok(Self { owner, slots })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MirCommitMaps {
    pub(crate) storage: MirCommitMap<StorageId>,
    pub(crate) values: MirCommitMap<ValueId>,
    pub(crate) blocks: MirCommitMap<BlockId>,
    pub(crate) path_conditions: MirCommitMap<PathConditionId>,
    pub(crate) optional_guards: MirCommitMap<OptionalGuardId>,
}

impl MirCommitMaps {
    fn build(inventory: &MirCallableEditInventory) -> Result<Self, MirRewriteError> {
        Ok(Self {
            storage: MirCommitMap::build_with_owner(inventory.callable, &inventory.storage)?,
            values: MirCommitMap::build_with_owner(inventory.callable, &inventory.values)?,
            blocks: MirCommitMap::build_with_owner(inventory.callable, &inventory.blocks)?,
            path_conditions: MirCommitMap::build_with_owner(
                inventory.callable,
                &inventory.path_conditions,
            )?,
            optional_guards: MirCommitMap::build_with_owner(
                inventory.callable,
                &inventory.optional_guards,
            )?,
        })
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct MirEntityChangeCount {
    pub(crate) retained: usize,
    pub(crate) inserted: usize,
    pub(crate) removed: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct MirRewriteChangeSummary {
    pub(crate) storage: MirEntityChangeCount,
    pub(crate) values: MirEntityChangeCount,
    pub(crate) blocks: MirEntityChangeCount,
    pub(crate) path_conditions: MirEntityChangeCount,
    pub(crate) optional_guards: MirEntityChangeCount,
    pub(crate) logical_expressions: MirEntityChangeCount,
}

impl MirRewriteChangeSummary {
    fn from_inventory(inventory: &MirCallableEditInventory) -> Self {
        Self {
            storage: identity_changes(&inventory.storage),
            values: identity_changes(&inventory.values),
            blocks: identity_changes(&inventory.blocks),
            path_conditions: identity_changes(&inventory.path_conditions),
            optional_guards: identity_changes(&inventory.optional_guards),
            logical_expressions: record_changes(&inventory.logical_expressions),
        }
    }

    pub(crate) fn accumulate(&mut self, other: Self) {
        self.storage.accumulate(other.storage);
        self.values.accumulate(other.values);
        self.blocks.accumulate(other.blocks);
        self.path_conditions.accumulate(other.path_conditions);
        self.optional_guards.accumulate(other.optional_guards);
        self.logical_expressions
            .accumulate(other.logical_expressions);
    }

    pub(crate) fn retained(self) -> usize {
        self.storage
            .retained
            .saturating_add(self.values.retained)
            .saturating_add(self.blocks.retained)
            .saturating_add(self.path_conditions.retained)
            .saturating_add(self.optional_guards.retained)
            .saturating_add(self.logical_expressions.retained)
    }

    pub(crate) fn inserted(self) -> usize {
        self.storage
            .inserted
            .saturating_add(self.values.inserted)
            .saturating_add(self.blocks.inserted)
            .saturating_add(self.path_conditions.inserted)
            .saturating_add(self.optional_guards.inserted)
            .saturating_add(self.logical_expressions.inserted)
    }

    pub(crate) fn removed(self) -> usize {
        self.storage
            .removed
            .saturating_add(self.values.removed)
            .saturating_add(self.blocks.removed)
            .saturating_add(self.path_conditions.removed)
            .saturating_add(self.optional_guards.removed)
            .saturating_add(self.logical_expressions.removed)
    }
}

impl MirEntityChangeCount {
    fn accumulate(&mut self, other: Self) {
        self.retained = self.retained.saturating_add(other.retained);
        self.inserted = self.inserted.saturating_add(other.inserted);
        self.removed = self.removed.saturating_add(other.removed);
    }
}

fn identity_changes<I>(inventory: &EditIdentityInventory<I>) -> MirEntityChangeCount {
    slot_changes(inventory.original_len, inventory.slots.iter().copied())
}

fn record_changes(inventory: &EditRecordInventory) -> MirEntityChangeCount {
    slot_changes(
        inventory.original_len,
        inventory.live.iter().copied().map(Some),
    )
}

fn slot_changes(
    original_len: usize,
    slots: impl IntoIterator<Item = Option<bool>>,
) -> MirEntityChangeCount {
    let mut changes = MirEntityChangeCount::default();
    for (index, state) in slots.into_iter().enumerate() {
        match (index < original_len, state) {
            (true, Some(true)) => changes.retained += 1,
            (true, Some(false)) => changes.removed += 1,
            (false, Some(true)) => changes.inserted += 1,
            (false, Some(false)) | (_, None) => {}
        }
    }
    changes
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct MirCommittedCallable {
    pub(super) callable: CallableId,
    pub(super) storage: Vec<MirStorage>,
    pub(super) values: Vec<MirValue>,
    pub(super) body: MirBody,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct MirCallableCommit {
    pub(super) callable: MirCommittedCallable,
    pub(super) maps: MirCommitMaps,
    pub(super) changes: MirRewriteChangeSummary,
}

pub(super) fn commit(edit: MirCallableEdit) -> Result<MirCallableCommit, MirRewriteError> {
    commit_with_attachments(edit, (), |(), _mapper| Ok(())).map(|(commit, ())| commit)
}

pub(super) fn commit_with_attachments<A>(
    edit: MirCallableEdit,
    mut attachments: A,
    map_attachments: impl FnOnce(&mut A, &mut CommitMapper<'_>) -> Result<(), MirRewriteError>,
) -> Result<(MirCallableCommit, A), MirRewriteError> {
    let inventory = edit.commit_inventory()?;
    let maps = MirCommitMaps::build(&inventory)?;
    let changes = MirRewriteChangeSummary::from_inventory(&inventory);
    let mut mapper = CommitMapper { maps: &maps };
    map_attachments(&mut attachments, &mut mapper)?;
    let mut candidate = edit.into_dense_candidate()?;
    map_common_local_identities(
        &mut candidate.storage,
        &mut candidate.values,
        &mut candidate.body,
        &mut mapper,
    )?;
    let callable = MirCommittedCallable {
        callable: candidate.callable,
        storage: candidate.storage,
        values: candidate.values,
        body: candidate.body,
    };
    Ok((
        MirCallableCommit {
            callable,
            maps,
            changes,
        },
        attachments,
    ))
}

pub(super) struct CommitMapper<'a> {
    maps: &'a MirCommitMaps,
}

impl MirLocalIdentityMapper for CommitMapper<'_> {
    type Error = MirRewriteError;

    fn map_storage(
        &mut self,
        site: MirLocalIdentitySite,
        identity: StorageId,
    ) -> Result<StorageId, Self::Error> {
        self.maps.storage.reference(identity, site)
    }

    fn map_value(
        &mut self,
        site: MirLocalIdentitySite,
        identity: ValueId,
    ) -> Result<ValueId, Self::Error> {
        self.maps.values.reference(identity, site)
    }

    fn map_block(
        &mut self,
        site: MirLocalIdentitySite,
        identity: BlockId,
    ) -> Result<BlockId, Self::Error> {
        self.maps.blocks.reference(identity, site)
    }

    fn map_path_condition(
        &mut self,
        site: MirLocalIdentitySite,
        identity: PathConditionId,
    ) -> Result<PathConditionId, Self::Error> {
        self.maps.path_conditions.reference(identity, site)
    }

    fn map_optional_guard(
        &mut self,
        site: MirLocalIdentitySite,
        identity: OptionalGuardId,
    ) -> Result<OptionalGuardId, Self::Error> {
        self.maps.optional_guards.reference(identity, site)
    }
}

#[cfg(test)]
mod tests;
