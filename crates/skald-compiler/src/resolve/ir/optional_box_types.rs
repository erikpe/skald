//! Deterministic resolved identities for shared boxes containing optionals.

use crate::{
    id_table::DenseIdTable,
    identity::{OptionalBoxTypeId, OptionalTypeId},
    source::Span,
};

use super::ResolvedObjectTarget;

/// Canonical metadata for one static `shared P?` target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedOptionalBoxType {
    pub id: OptionalBoxTypeId,
    /// Exact canonical optional wrapper stored in the allocation.
    pub optional: OptionalTypeId,
    /// Number of optional constructors between the box and its leaf.
    pub optional_depth: usize,
    /// Static object view at the leaf, when this is an object box.
    pub object_leaf: Option<ResolvedObjectTarget>,
    /// First deterministic source occurrence, used by availability diagnostics.
    pub span: Span,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ResolvedOptionalBoxTypeTable {
    entries: DenseIdTable<OptionalBoxTypeId, ResolvedOptionalBoxType>,
}

impl ResolvedOptionalBoxTypeTable {
    pub(crate) fn new(entries: Vec<ResolvedOptionalBoxType>) -> Self {
        Self {
            entries: DenseIdTable::new(entries, |entry| entry.id),
        }
    }

    pub fn get(&self, id: OptionalBoxTypeId) -> Option<&ResolvedOptionalBoxType> {
        self.entries.get(id, |entry| entry.id)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &ResolvedOptionalBoxType> {
        self.entries.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
