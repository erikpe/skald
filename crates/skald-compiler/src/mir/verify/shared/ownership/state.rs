use std::collections::{HashMap, HashSet};

use crate::{
    identity::LiteralDataId,
    mir::{MirDefinitionRef, MirPlace, MirSharedAllocationMode, MirType, StorageId},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum AllocationState {
    Allocated(MirSharedAllocationMode),
    Initialized,
    Published,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct SharedState {
    pub(super) allocations: HashMap<StorageId, AllocationState>,
    pub(super) live_owners: HashSet<StorageId>,
    pub(super) owner_origins: HashMap<StorageId, StorageId>,
    /// Static literal owners may only be consumed by exact string publication.
    pub(super) static_owners: HashMap<StorageId, LiteralDataId>,
    pub(super) released_owners: HashSet<StorageId>,
    /// Checked-view carrier to the shared owner that keeps its payload live.
    pub(super) active_checked_views: HashMap<StorageId, StorageId>,
    pub(super) initialized_fields: HashSet<MirPlace>,
    pub(super) pending_full_expression_boundary: bool,
}

impl SharedState {
    pub(super) fn at_entry(function: MirDefinitionRef<'_>) -> Self {
        let mut state = Self::default();
        for parameter in function.parameters() {
            if function
                .storage(*parameter)
                .is_some_and(|storage| matches!(storage.ty, MirType::Shared(_)))
            {
                state.live_owners.insert(*parameter);
                state.owner_origins.insert(*parameter, *parameter);
            }
        }
        state
    }

    pub(super) fn reset_storage(&mut self, storage: StorageId) {
        self.allocations.remove(&storage);
        self.live_owners.remove(&storage);
        self.owner_origins.remove(&storage);
        self.static_owners.remove(&storage);
        self.released_owners.remove(&storage);
        self.active_checked_views
            .retain(|carrier, owner| *carrier != storage && *owner != storage);
        self.initialized_fields
            .retain(|place| place.base.storage() != storage);
    }

    pub(super) fn same_live_state(&self, other: &Self) -> bool {
        self.allocations == other.allocations
            && self.live_owners == other.live_owners
            && self.owner_origins == other.owner_origins
            && self.static_owners == other.static_owners
            && self.active_checked_views == other.active_checked_views
            && self.initialized_fields == other.initialized_fields
            && self.pending_full_expression_boundary == other.pending_full_expression_boundary
    }
}
