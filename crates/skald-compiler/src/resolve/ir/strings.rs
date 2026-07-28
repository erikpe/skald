//! Validated string language-item metadata and decoded literal data.

use crate::{
    id_table::DenseIdTable,
    identity::{ClassId, FieldId, LiteralDataId},
    source::Span,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedStringLanguageItem {
    pub class: ClassId,
    pub storage_field: FieldId,
    pub start_field: FieldId,
    pub length_field: FieldId,
    pub declaration_span: Span,
    pub requiring_literal_spans: Vec<Span>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedLiteralData {
    pub id: LiteralDataId,
    pub bytes: Vec<u8>,
    pub span: Span,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ResolvedLiteralDataTable {
    entries: DenseIdTable<LiteralDataId, ResolvedLiteralData>,
}

impl ResolvedLiteralDataTable {
    pub(crate) fn new(entries: Vec<ResolvedLiteralData>) -> Self {
        Self {
            entries: DenseIdTable::new(entries, |entry| entry.id),
        }
    }

    pub fn get(&self, id: LiteralDataId) -> Option<&ResolvedLiteralData> {
        self.entries.get(id, |entry| entry.id)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &ResolvedLiteralData> {
        self.entries.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
