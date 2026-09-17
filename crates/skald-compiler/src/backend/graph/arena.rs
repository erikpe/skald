//! Checked identity allocation and lookup; no graph/value semantics live here.

use std::marker::PhantomData;

use crate::backend::plan::{CallableBinding, LirCallableId, PlanError};
use crate::id_table::{DenseId, DenseIdTable};

macro_rules! local_domain {
    ($domain:ident, $id:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        #[cfg_attr(not(test), allow(dead_code))]
        pub(in crate::backend) enum $domain {}
        #[cfg_attr(not(test), allow(dead_code))]
        pub(in crate::backend) type $id = LocalId<$domain>;
    };
}

local_domain!(LoweredBlock, LoweredBlockId);
local_domain!(LoweredValue, LoweredValueId);
local_domain!(LoweredObject, LoweredObjectId);
local_domain!(SelectedBlock, SelectedBlockId);
local_domain!(SelectedValue, SelectedValueId);
local_domain!(SelectedObject, SelectedObjectId);

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct LocalId<K> {
    callable: LirCallableId,
    index: usize,
    kind: PhantomData<fn(K) -> K>,
}

#[cfg_attr(not(test), allow(dead_code))]
impl<K: Copy + Eq> DenseId for LocalId<K> {
    fn index(self) -> usize {
        self.index
    }
}

#[cfg_attr(not(test), allow(dead_code))]
impl<K: Copy> LocalId<K> {
    pub(in crate::backend) const fn callable(self) -> LirCallableId {
        self.callable
    }
    pub(in crate::backend) const fn index(self) -> usize {
        self.index
    }
}

#[derive(Clone, Copy)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct LocalHandle<'plan, I> {
    owner: CallableBinding<'plan>,
    id: I,
}

#[cfg_attr(not(test), allow(dead_code))]
struct Entry<I, T> {
    id: I,
    value: T,
}

#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct OwnedArena<'plan, I, T> {
    owner: CallableBinding<'plan>,
    entries: DenseIdTable<I, Entry<I, T>>,
}

#[cfg_attr(not(test), allow(dead_code))]
impl<'plan, K: Copy + Eq, T> OwnedArena<'plan, LocalId<K>, T> {
    pub(in crate::backend) fn new(owner: CallableBinding<'plan>) -> Self {
        Self {
            owner,
            entries: DenseIdTable::default(),
        }
    }

    pub(in crate::backend) fn push(
        &mut self,
        value: T,
    ) -> Result<LocalHandle<'plan, LocalId<K>>, PlanError> {
        let index = checked_index(self.entries.len())?;
        let id = LocalId {
            callable: self.owner.key(),
            index,
            kind: PhantomData,
        };
        self.entries.extend([Entry { id, value }], |entry| entry.id);
        Ok(LocalHandle {
            owner: self.owner,
            id,
        })
    }

    pub(in crate::backend) fn get(
        &self,
        handle: LocalHandle<'plan, LocalId<K>>,
    ) -> Result<&T, PlanError> {
        self.owner.require_same_owner(handle.owner)?;
        if handle.id.callable != self.owner.key() {
            return Err(PlanError::WrongOwner);
        }
        self.entries
            .get(handle.id, |entry| entry.id)
            .map(|entry| &entry.value)
            .ok_or(PlanError::OutOfBounds)
    }

    pub(in crate::backend) fn iter(&self) -> impl ExactSizeIterator<Item = (LocalId<K>, &T)> {
        self.entries.iter().map(|entry| (entry.id, &entry.value))
    }
}

#[cfg_attr(not(test), allow(dead_code))]
impl<I: Copy> LocalHandle<'_, I> {
    pub(in crate::backend) const fn id(self) -> I {
        self.id
    }
}

#[cfg_attr(not(test), allow(dead_code))]
fn checked_index(len: usize) -> Result<usize, PlanError> {
    len.checked_add(1).ok_or(PlanError::SizeOverflow)?;
    Ok(len)
}

#[cfg(test)]
mod tests;
