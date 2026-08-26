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
            .retain(|place| place.base.local_storage() != Some(storage));
    }

    /// Join two paths whose live owner set agrees while forgetting allocation
    /// identity for owners that were replaced on only one path.
    ///
    /// The owner remains valid, but exact dynamic provenance is no longer
    /// available after the join. Using the owner's own shared storage as the
    /// conservative origin preserves liveness without pretending that two
    /// different allocations are identical.
    pub(super) fn merge_live_state(&mut self, other: &Self) -> bool {
        if self.allocations != other.allocations
            || self.live_owners != other.live_owners
            || self.owner_origins.keys().collect::<HashSet<_>>()
                != other.owner_origins.keys().collect::<HashSet<_>>()
            || self.static_owners != other.static_owners
            || self.active_checked_views != other.active_checked_views
            || self.initialized_fields != other.initialized_fields
            || self.pending_full_expression_boundary != other.pending_full_expression_boundary
        {
            return false;
        }
        for (&owner, &incoming_origin) in &other.owner_origins {
            if self.owner_origins.get(&owner) != Some(&incoming_origin) {
                self.owner_origins.insert(owner, owner);
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use crate::identity::{CallableId, FunctionId};

    use super::*;

    #[test]
    fn joining_replaced_live_owner_forgets_only_its_allocation_identity() {
        let callable = CallableId::Function(FunctionId::new(0));
        let owner = StorageId::new(callable, 0);
        let first = StorageId::new(callable, 1);
        let second = StorageId::new(callable, 2);
        let mut left = SharedState::default();
        left.live_owners.insert(owner);
        left.owner_origins.insert(owner, first);
        let mut right = left.clone();
        right.owner_origins.insert(owner, second);

        assert!(left.merge_live_state(&right));
        assert_eq!(left.owner_origins.get(&owner), Some(&owner));
        assert!(left.live_owners.contains(&owner));
    }
}
