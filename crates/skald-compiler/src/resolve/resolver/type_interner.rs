//! Deterministic bottom-up interning of recursive resolved value types.

use std::collections::HashMap;

use crate::identity::{ArrayTypeId, OptionalTypeId};

use super::{
    ResolvedArrayType, ResolvedArrayTypeTable, ResolvedOptionalType, ResolvedOptionalTypeTable,
    ResolvedType, ResolvedTypeKind,
};

/// Owns canonical identities for the mutually recursive array/optional graph.
///
/// Resolution interns children before parents. `ResolvedTypeKind` contains
/// only semantic identities, so source spans never affect equality or keys.
#[derive(Default)]
pub(super) struct ResolvedTypeInterner {
    array_ids: HashMap<ResolvedTypeKind, ArrayTypeId>,
    arrays: Vec<ResolvedArrayType>,
    optional_ids: HashMap<ResolvedTypeKind, OptionalTypeId>,
    optionals: Vec<ResolvedOptionalType>,
}

impl ResolvedTypeInterner {
    pub(super) fn intern_array(&mut self, element: ResolvedType) -> ArrayTypeId {
        if let Some(id) = self.array_ids.get(&element.kind) {
            return *id;
        }

        let id = ArrayTypeId::new(self.arrays.len());
        self.array_ids.insert(element.kind, id);
        self.arrays.push(ResolvedArrayType { id, element });
        id
    }

    pub(super) fn intern_optional(&mut self, payload: ResolvedType) -> OptionalTypeId {
        if let Some(id) = self.optional_ids.get(&payload.kind) {
            return *id;
        }

        let id = OptionalTypeId::new(self.optionals.len());
        self.optional_ids.insert(payload.kind, id);
        self.optionals.push(ResolvedOptionalType { id, payload });
        id
    }

    pub(super) fn array(&self, id: ArrayTypeId) -> Option<&ResolvedArrayType> {
        self.arrays.get(id.index()).filter(|entry| entry.id == id)
    }

    pub(super) fn optional(&self, id: OptionalTypeId) -> Option<&ResolvedOptionalType> {
        self.optionals
            .get(id.index())
            .filter(|entry| entry.id == id)
    }

    pub(super) fn finish(self) -> (ResolvedArrayTypeTable, ResolvedOptionalTypeTable) {
        (
            ResolvedArrayTypeTable::new(self.arrays),
            ResolvedOptionalTypeTable::new(self.optionals),
        )
    }
}
