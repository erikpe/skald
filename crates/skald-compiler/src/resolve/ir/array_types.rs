//! Canonical recursive array identities produced during resolution.

use crate::{id_table::DenseIdTable, identity::ArrayTypeId};

use super::ResolvedType;

/// One canonical array type, identified by its exact resolved element type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedArrayType {
    pub id: ArrayTypeId,
    pub element: ResolvedType,
}

/// Dense, deterministic storage for canonical array types.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ResolvedArrayTypeTable {
    entries: DenseIdTable<ArrayTypeId, ResolvedArrayType>,
}

impl ResolvedArrayTypeTable {
    pub(crate) fn new(entries: Vec<ResolvedArrayType>) -> Self {
        Self {
            entries: DenseIdTable::new(entries, |entry| entry.id),
        }
    }

    pub fn get(&self, id: ArrayTypeId) -> Option<&ResolvedArrayType> {
        self.entries.get(id, |entry| entry.id)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &ResolvedArrayType> {
        self.entries.iter()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
