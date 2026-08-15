//! Canonical typed function signatures retained by HIR.

use crate::{id_table::DenseIdTable, identity::FunctionTypeId, source::Span};

use super::Type;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum HirFunctionTypeParameterMode {
    Value,
    ReadOnlyAlias,
    MutableAlias,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirFunctionTypeParameter {
    pub mode: HirFunctionTypeParameterMode,
    pub ty: Type,
    pub span: Span,
}

/// One canonical, exact function-value signature.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirFunctionType {
    pub id: FunctionTypeId,
    pub parameters: Vec<HirFunctionTypeParameter>,
    pub result: Type,
    pub span: Span,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HirFunctionTypeTable {
    entries: DenseIdTable<FunctionTypeId, HirFunctionType>,
}

impl HirFunctionTypeTable {
    pub(crate) fn new(entries: Vec<HirFunctionType>) -> Self {
        Self {
            entries: DenseIdTable::new(entries, |entry| entry.id),
        }
    }

    pub fn get(&self, id: FunctionTypeId) -> Option<&HirFunctionType> {
        self.entries.get(id, |entry| entry.id)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &HirFunctionType> {
        self.entries.iter()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
