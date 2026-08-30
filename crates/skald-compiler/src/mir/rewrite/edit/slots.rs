use std::marker::PhantomData;

use crate::identity::CallableId;

use super::super::super::{
    BlockId, MirBasicBlock, MirPathCondition, MirStorage, MirValue, PathConditionId, StorageId,
    ValueId,
};
use super::super::{error::MirRewriteError, MirLocalIdentity};

pub(super) trait EditSlotId: Copy + Eq {
    fn new(owner: CallableId, index: usize) -> Self;
    fn callable(self) -> CallableId;
    fn index(self) -> usize;
    fn local_identity(self) -> MirLocalIdentity;
}

macro_rules! edit_slot_id {
    ($identity:ty, $variant:ident) => {
        impl EditSlotId for $identity {
            fn new(owner: CallableId, index: usize) -> Self {
                <$identity>::new(owner, index)
            }

            fn callable(self) -> CallableId {
                self.callable()
            }

            fn index(self) -> usize {
                self.index()
            }

            fn local_identity(self) -> MirLocalIdentity {
                MirLocalIdentity::$variant(self)
            }
        }
    };
}

edit_slot_id!(StorageId, Storage);
edit_slot_id!(ValueId, Value);
edit_slot_id!(BlockId, Block);
edit_slot_id!(PathConditionId, PathCondition);

pub(super) trait EditSlotEntry<I> {
    fn edit_slot_id(&self) -> I;
}

impl EditSlotEntry<StorageId> for MirStorage {
    fn edit_slot_id(&self) -> StorageId {
        self.id
    }
}

impl EditSlotEntry<ValueId> for MirValue {
    fn edit_slot_id(&self) -> ValueId {
        self.id
    }
}

impl EditSlotEntry<BlockId> for MirBasicBlock {
    fn edit_slot_id(&self) -> BlockId {
        self.id
    }
}

impl EditSlotEntry<PathConditionId> for MirPathCondition {
    fn edit_slot_id(&self) -> PathConditionId {
        self.id
    }
}

/// Stable, monotonically allocated edit slots with explicit tombstones.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SparseSlots<I, T> {
    owner: CallableId,
    entries: Vec<Option<T>>,
    identity: PhantomData<I>,
}

impl<I, T> SparseSlots<I, T>
where
    I: EditSlotId,
    T: EditSlotEntry<I>,
{
    pub(super) fn from_dense(owner: CallableId, entries: Vec<T>) -> Result<Self, MirRewriteError> {
        for (index, entry) in entries.iter().enumerate() {
            let expected = I::new(owner, index);
            let actual = entry.edit_slot_id();
            if actual.callable() != owner {
                return Err(MirRewriteError::ForeignIdentity {
                    expected: owner,
                    identity: actual.local_identity(),
                });
            }
            if actual != expected {
                return Err(MirRewriteError::DeclarationIdentityMismatch {
                    expected: expected.local_identity(),
                    actual: actual.local_identity(),
                });
            }
        }
        Ok(Self {
            owner,
            entries: entries.into_iter().map(Some).collect(),
            identity: PhantomData,
        })
    }

    pub(super) const fn owner(&self) -> CallableId {
        self.owner
    }

    pub(super) fn next_id(&self) -> I {
        I::new(self.owner, self.entries.len())
    }

    pub(super) fn append(&mut self, entry: T) -> Result<I, MirRewriteError> {
        let expected = self.validate_next(&entry)?;
        self.entries.push(Some(entry));
        Ok(expected)
    }

    pub(super) fn validate_next(&self, entry: &T) -> Result<I, MirRewriteError> {
        let expected = self.next_id();
        let actual = entry.edit_slot_id();
        if actual.callable() != self.owner {
            return Err(MirRewriteError::ForeignIdentity {
                expected: self.owner,
                identity: actual.local_identity(),
            });
        }
        if actual == expected {
            Ok(expected)
        } else {
            Err(MirRewriteError::DeclarationIdentityMismatch {
                expected: expected.local_identity(),
                actual: actual.local_identity(),
            })
        }
    }

    pub(super) fn append_prevalidated(&mut self, identity: I, entry: T) {
        debug_assert!(identity == self.next_id());
        self.entries.push(Some(entry));
    }

    pub(super) fn allocate_with(
        &mut self,
        build: impl FnOnce(I) -> T,
    ) -> Result<I, MirRewriteError> {
        let identity = self.next_id();
        self.append(build(identity))
    }

    pub(super) fn get(&self, identity: I) -> Result<&T, MirRewriteError> {
        self.validate_owner(identity)?;
        match self.entries.get(identity.index()) {
            Some(Some(entry)) => Ok(entry),
            Some(None) => Err(MirRewriteError::DeletedIdentity {
                identity: identity.local_identity(),
            }),
            None => Err(MirRewriteError::UnknownIdentity {
                identity: identity.local_identity(),
            }),
        }
    }

    pub(super) fn remove(&mut self, identity: I) -> Result<T, MirRewriteError> {
        self.validate_owner(identity)?;
        match self.entries.get_mut(identity.index()) {
            Some(slot @ Some(_)) => Ok(slot.take().expect("live edit slot was checked")),
            Some(None) => Err(MirRewriteError::DeletedIdentity {
                identity: identity.local_identity(),
            }),
            None => Err(MirRewriteError::UnknownIdentity {
                identity: identity.local_identity(),
            }),
        }
    }

    pub(super) fn live_ids(&self) -> impl Iterator<Item = I> + '_ {
        self.entries
            .iter()
            .enumerate()
            .filter_map(|(index, entry)| entry.as_ref().map(|_| I::new(self.owner, index)))
    }

    pub(super) fn live_entries(&self) -> impl Iterator<Item = &T> {
        self.entries.iter().filter_map(Option::as_ref)
    }

    fn validate_owner(&self, identity: I) -> Result<(), MirRewriteError> {
        if identity.callable() == self.owner {
            Ok(())
        } else {
            Err(MirRewriteError::ForeignIdentity {
                expected: self.owner,
                identity: identity.local_identity(),
            })
        }
    }
}
