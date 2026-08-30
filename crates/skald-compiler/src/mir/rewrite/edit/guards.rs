use std::collections::BTreeSet;

use crate::identity::CallableId;

use super::super::super::{MirBody, OptionalGuardId};
use super::super::{
    error::MirRewriteError, map::map_body_local_identities, MirLocalIdentity,
    MirLocalIdentityMapper, MirLocalIdentitySite,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GuardSlot {
    Unallocated,
    Live,
    Deleted,
}

/// Declaration-like state for optional guards, which committed MIR represents
/// only through paired references.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct OptionalGuardRegistry {
    owner: CallableId,
    original_len: usize,
    slots: Vec<GuardSlot>,
}

impl OptionalGuardRegistry {
    pub(super) fn discover(owner: CallableId, body: &mut MirBody) -> Result<Self, MirRewriteError> {
        let mut collector = GuardCollector {
            owner,
            guards: BTreeSet::new(),
        };
        map_body_local_identities(body, &mut collector)?;
        let slot_count = collector
            .guards
            .last()
            .map_or(0, |identity| identity.index() + 1);
        let mut slots = vec![GuardSlot::Unallocated; slot_count];
        for identity in collector.guards {
            slots[identity.index()] = GuardSlot::Live;
        }
        Ok(Self {
            owner,
            original_len: slot_count,
            slots,
        })
    }

    pub(super) fn allocate(&mut self) -> OptionalGuardId {
        let identity = OptionalGuardId::new(self.owner, self.slots.len());
        self.slots.push(GuardSlot::Live);
        identity
    }

    pub(super) fn get(&self, identity: OptionalGuardId) -> Result<(), MirRewriteError> {
        self.validate_owner(identity)?;
        match self.slots.get(identity.index()) {
            Some(GuardSlot::Live) => Ok(()),
            Some(GuardSlot::Deleted) => Err(MirRewriteError::DeletedIdentity {
                identity: MirLocalIdentity::OptionalGuard(identity),
            }),
            Some(GuardSlot::Unallocated) | None => Err(MirRewriteError::UnknownIdentity {
                identity: MirLocalIdentity::OptionalGuard(identity),
            }),
        }
    }

    pub(super) fn remove(&mut self, identity: OptionalGuardId) -> Result<(), MirRewriteError> {
        self.validate_owner(identity)?;
        match self.slots.get_mut(identity.index()) {
            Some(slot @ GuardSlot::Live) => {
                *slot = GuardSlot::Deleted;
                Ok(())
            }
            Some(GuardSlot::Deleted) => Err(MirRewriteError::DeletedIdentity {
                identity: MirLocalIdentity::OptionalGuard(identity),
            }),
            Some(GuardSlot::Unallocated) | None => Err(MirRewriteError::UnknownIdentity {
                identity: MirLocalIdentity::OptionalGuard(identity),
            }),
        }
    }

    pub(super) fn live_ids(&self) -> impl Iterator<Item = OptionalGuardId> + '_ {
        self.slots
            .iter()
            .enumerate()
            .filter(|(_, slot)| **slot == GuardSlot::Live)
            .map(|(index, _)| OptionalGuardId::new(self.owner, index))
    }

    pub(super) const fn original_len(&self) -> usize {
        self.original_len
    }

    pub(super) fn slot_liveness(&self) -> impl Iterator<Item = Option<bool>> + '_ {
        self.slots.iter().map(|slot| match slot {
            GuardSlot::Unallocated => None,
            GuardSlot::Live => Some(true),
            GuardSlot::Deleted => Some(false),
        })
    }

    #[cfg(test)]
    pub(super) fn forget_for_test(&mut self, identity: OptionalGuardId) {
        self.slots[identity.index()] = GuardSlot::Unallocated;
    }

    fn validate_owner(&self, identity: OptionalGuardId) -> Result<(), MirRewriteError> {
        if identity.callable() == self.owner {
            Ok(())
        } else {
            Err(MirRewriteError::ForeignIdentity {
                expected: self.owner,
                identity: MirLocalIdentity::OptionalGuard(identity),
            })
        }
    }
}

struct GuardCollector {
    owner: CallableId,
    guards: BTreeSet<OptionalGuardId>,
}

impl MirLocalIdentityMapper for GuardCollector {
    type Error = MirRewriteError;

    fn map_optional_guard(
        &mut self,
        _site: MirLocalIdentitySite,
        identity: OptionalGuardId,
    ) -> Result<OptionalGuardId, Self::Error> {
        if identity.callable() == self.owner {
            self.guards.insert(identity);
        }
        Ok(identity)
    }
}
