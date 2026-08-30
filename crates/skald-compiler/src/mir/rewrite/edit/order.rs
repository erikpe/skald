use std::collections::BTreeSet;

use crate::identity::CallableId;

use super::{super::error::MirRewriteError, slots::EditSlotId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::mir::rewrite) enum OrderPlacement<I> {
    Append,
    Before(I),
    After(I),
}

/// Explicit deterministic live order independent from sparse allocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct LiveOrder<I> {
    owner: CallableId,
    entries: Vec<I>,
}

impl<I: EditSlotId + Ord> LiveOrder<I> {
    pub(super) fn complete(
        owner: CallableId,
        live: impl IntoIterator<Item = I>,
        entries: Vec<I>,
    ) -> Result<Self, MirRewriteError> {
        let live: BTreeSet<_> = live.into_iter().collect();
        let mut ordered = BTreeSet::new();
        for identity in entries.iter().copied() {
            validate_owner(owner, identity)?;
            if !ordered.insert(identity) {
                return Err(MirRewriteError::DuplicateOrderIdentity {
                    identity: identity.local_identity(),
                });
            }
        }
        for identity in live.iter().copied() {
            validate_owner(owner, identity)?;
            if !ordered.contains(&identity) {
                return Err(MirRewriteError::MissingOrderIdentity {
                    identity: identity.local_identity(),
                });
            }
        }
        if let Some(identity) = ordered.difference(&live).copied().next() {
            return Err(MirRewriteError::UnknownIdentity {
                identity: identity.local_identity(),
            });
        }
        Ok(Self { owner, entries })
    }

    pub(super) fn entries(&self) -> &[I] {
        &self.entries
    }

    pub(super) fn contains(&self, identity: I) -> bool {
        self.entries.contains(&identity)
    }

    pub(super) fn insert(
        &mut self,
        identity: I,
        placement: OrderPlacement<I>,
    ) -> Result<(), MirRewriteError> {
        validate_owner(self.owner, identity)?;
        if self.entries.contains(&identity) {
            return Err(MirRewriteError::DuplicateOrderIdentity {
                identity: identity.local_identity(),
            });
        }
        let index = match placement {
            OrderPlacement::Append => self.entries.len(),
            OrderPlacement::Before(anchor) => self.anchor_index(anchor)?,
            OrderPlacement::After(anchor) => self.anchor_index(anchor)? + 1,
        };
        self.entries.insert(index, identity);
        Ok(())
    }

    pub(super) fn remove(&mut self, identity: I) -> Result<(), MirRewriteError> {
        validate_owner(self.owner, identity)?;
        let Some(index) = self.entries.iter().position(|entry| *entry == identity) else {
            return Err(MirRewriteError::MissingOrderIdentity {
                identity: identity.local_identity(),
            });
        };
        self.entries.remove(index);
        Ok(())
    }

    fn anchor_index(&self, anchor: I) -> Result<usize, MirRewriteError> {
        validate_owner(self.owner, anchor)?;
        self.entries
            .iter()
            .position(|entry| *entry == anchor)
            .ok_or(MirRewriteError::MissingOrderIdentity {
                identity: anchor.local_identity(),
            })
    }
}

fn validate_owner<I: EditSlotId>(expected: CallableId, identity: I) -> Result<(), MirRewriteError> {
    if identity.callable() == expected {
        Ok(())
    } else {
        Err(MirRewriteError::ForeignIdentity {
            expected,
            identity: identity.local_identity(),
        })
    }
}
