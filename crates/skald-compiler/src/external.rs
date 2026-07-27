//! Compilation-wide foreign-symbol linkage selected during resolution.

use crate::{
    id_table::DenseIdTable,
    identity::{ExternalLinkId, FunctionId},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalLink {
    pub id: ExternalLinkId,
    pub symbol: String,
    pub declarations: Vec<FunctionId>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ExternalLinkTable {
    entries: DenseIdTable<ExternalLinkId, ExternalLink>,
}

impl ExternalLinkTable {
    pub(crate) fn new(entries: Vec<ExternalLink>) -> Self {
        Self {
            entries: DenseIdTable::new(entries, |entry| entry.id),
        }
    }

    pub fn get(&self, id: ExternalLinkId) -> Option<&ExternalLink> {
        self.entries.get(id, |entry| entry.id)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &ExternalLink> {
        self.entries.iter()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[cfg(test)]
    pub(crate) fn entries_mut_for_test(&mut self) -> &mut [ExternalLink] {
        self.entries.entries_mut_for_test()
    }
}
