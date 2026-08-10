//! Canonical recursive optional identities produced during resolution.

use crate::{id_table::DenseIdTable, identity::OptionalTypeId};

use super::ResolvedType;

/// One canonical optional type, identified by its exact resolved payload type.
///
/// Source spans describe the first deterministic occurrence of the identity.
/// They are retained for diagnostics but do not participate in interning.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedOptionalType {
    pub id: OptionalTypeId,
    pub payload: ResolvedType,
}

/// Dense, deterministic storage for canonical optional types.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ResolvedOptionalTypeTable {
    entries: DenseIdTable<OptionalTypeId, ResolvedOptionalType>,
}

impl ResolvedOptionalTypeTable {
    pub(crate) fn new(entries: Vec<ResolvedOptionalType>) -> Self {
        Self {
            entries: DenseIdTable::new(entries, |entry| entry.id),
        }
    }

    pub fn get(&self, id: OptionalTypeId) -> Option<&ResolvedOptionalType> {
        self.entries.get(id, |entry| entry.id)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &ResolvedOptionalType> {
        self.entries.iter()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
