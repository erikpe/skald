//! Target-independent string language-item and literal-data representation.

use crate::{
    id_table::DenseIdTable,
    identity::{ArrayTypeId, ClassId, FieldId, LiteralDataId},
    source::Span,
};

use super::{MirPlace, StorageId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MirStringLanguageItem {
    pub class: ClassId,
    pub storage_field: FieldId,
    pub start_field: FieldId,
    pub length_field: FieldId,
    pub storage_array: ArrayTypeId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirStaticAllocationOrigin {
    Immortal,
    /// Reserved malformed/foreign MIR provenance. Literal backing must be immortal.
    Unspecified,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirStaticDataMutability {
    Immutable,
    /// Reserved malformed/foreign MIR state. Literal backing must be immutable.
    Mutable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirLiteralData {
    pub id: LiteralDataId,
    pub bytes: Vec<u8>,
    pub array: ArrayTypeId,
    pub length: u64,
    pub mutability: MirStaticDataMutability,
    pub origin: MirStaticAllocationOrigin,
    pub span: Span,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MirLiteralDataTable {
    entries: DenseIdTable<LiteralDataId, MirLiteralData>,
}

impl MirLiteralDataTable {
    pub(crate) fn new(entries: Vec<MirLiteralData>) -> Self {
        Self {
            entries: DenseIdTable::new(entries, |entry| entry.id),
        }
    }

    pub fn get(&self, id: LiteralDataId) -> Option<&MirLiteralData> {
        self.entries.get(id, |entry| entry.id)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &MirLiteralData> {
        self.entries.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[cfg(test)]
    pub(crate) fn entries_mut_for_test(&mut self) -> &mut [MirLiteralData] {
        self.entries.entries_mut_for_test()
    }
}

/// Publishes one complete exact string descriptor after consuming its static
/// backing owner. Field identities are semantic; no target offsets appear.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirStringInitialize {
    pub destination: MirPlace,
    pub data: LiteralDataId,
    pub backing: StorageId,
    pub class: ClassId,
    pub storage_field: FieldId,
    pub start_field: FieldId,
    pub length_field: FieldId,
    pub start: u64,
    pub length: u64,
    pub span: Span,
}
