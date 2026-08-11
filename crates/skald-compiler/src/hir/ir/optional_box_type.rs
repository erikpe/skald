//! Canonical typed identities for shared boxes containing optional values.

use crate::{
    id_table::DenseIdTable,
    identity::{OptionalBoxTypeId, OptionalTypeId},
    source::Span,
};

use super::HirViewTarget;

/// The static target of one `shared P?` owner family.
///
/// Constructible exact targets retain their ordinary optional identity.
/// Interface and `Obj` leaves are box-only views and therefore have no
/// standalone inline-optional identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirOptionalBoxType {
    pub id: OptionalBoxTypeId,
    pub exact_optional: Option<OptionalTypeId>,
    pub optional_depth: usize,
    pub object_view: Option<HirViewTarget>,
    pub span: Span,
}

impl HirOptionalBoxType {
    pub const fn is_exact(&self) -> bool {
        self.exact_optional.is_some()
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HirOptionalBoxTypeTable {
    entries: DenseIdTable<OptionalBoxTypeId, HirOptionalBoxType>,
}

impl HirOptionalBoxTypeTable {
    pub(crate) fn new(entries: Vec<HirOptionalBoxType>) -> Self {
        Self {
            entries: DenseIdTable::new(entries, |entry| entry.id),
        }
    }

    pub fn get(&self, id: OptionalBoxTypeId) -> Option<&HirOptionalBoxType> {
        self.entries.get(id, |entry| entry.id)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &HirOptionalBoxType> {
        self.entries.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
