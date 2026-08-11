//! Canonical MIR identities for shared allocations containing optional values.

use crate::{
    id_table::DenseIdTable,
    identity::{ClassId, OptionalBoxTypeId, OptionalTypeId},
    source::Span,
};

use super::MirViewTarget;

/// The target-independent metadata carried by one `shared P?` owner family.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirOptionalBoxType {
    pub id: OptionalBoxTypeId,
    /// The physical wrapper type for constructible exact targets. Interface
    /// and `Obj` entries are owner views and deliberately have no inline type.
    pub exact_optional: Option<OptionalTypeId>,
    /// Exact class whose dispatch entries are retained when this constructible
    /// identity supplies an allocation descriptor. Interface and `Obj`
    /// view-only identities have no descriptor of their own.
    pub exact_dynamic_class: Option<ClassId>,
    pub optional_depth: usize,
    pub object_view: Option<MirViewTarget>,
    pub span: Span,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MirOptionalBoxTypeTable {
    entries: DenseIdTable<OptionalBoxTypeId, MirOptionalBoxType>,
}

impl MirOptionalBoxTypeTable {
    pub(crate) fn new(entries: Vec<MirOptionalBoxType>) -> Self {
        Self {
            entries: DenseIdTable::new(entries, |entry| entry.id),
        }
    }

    pub fn get(&self, id: OptionalBoxTypeId) -> Option<&MirOptionalBoxType> {
        self.entries.get(id, |entry| entry.id)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &MirOptionalBoxType> {
        self.entries.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[cfg(test)]
    pub(crate) fn entries_mut_for_test(&mut self) -> &mut [MirOptionalBoxType] {
        self.entries.entries_mut_for_test()
    }
}
