//! Canonical closed function signatures retained by MIR.

use crate::{id_table::DenseIdTable, identity::FunctionTypeId, source::Span};

use super::{MirParameter, MirType};

/// One exact receiverless signature for capture-free function values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirFunctionType {
    pub id: FunctionTypeId,
    pub parameters: Vec<MirParameter>,
    pub result: MirType,
    pub span: Span,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MirFunctionTypeTable {
    entries: DenseIdTable<FunctionTypeId, MirFunctionType>,
}

impl MirFunctionTypeTable {
    pub(crate) fn new(entries: Vec<MirFunctionType>) -> Self {
        Self {
            entries: DenseIdTable::new(entries, |entry| entry.id),
        }
    }

    pub fn get(&self, id: FunctionTypeId) -> Option<&MirFunctionType> {
        self.entries.get(id, |entry| entry.id)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &MirFunctionType> {
        self.entries.iter()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[cfg(test)]
    pub(crate) fn entries_mut_for_test(&mut self) -> &mut [MirFunctionType] {
        self.entries.entries_mut_for_test()
    }
}
