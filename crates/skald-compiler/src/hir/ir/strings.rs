//! Typed string language-item metadata and canonical literal data.

use crate::{
    id_table::DenseIdTable,
    identity::{ClassId, FieldId, LiteralDataId},
    source::Span,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirStringLanguageItem {
    pub class: ClassId,
    pub storage_field: FieldId,
    pub start_field: FieldId,
    pub length_field: FieldId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirLiteralData {
    pub id: LiteralDataId,
    pub bytes: Vec<u8>,
    pub span: Span,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HirLiteralDataTable {
    entries: DenseIdTable<LiteralDataId, HirLiteralData>,
}

impl HirLiteralDataTable {
    pub(crate) fn new(entries: Vec<HirLiteralData>) -> Self {
        Self {
            entries: DenseIdTable::new(entries, |entry| entry.id),
        }
    }

    pub fn get(&self, id: LiteralDataId) -> Option<&HirLiteralData> {
        self.entries.get(id, |entry| entry.id)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &HirLiteralData> {
        self.entries.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HirStringLiteral {
    pub data: LiteralDataId,
    pub class: ClassId,
    pub span: Span,
}
