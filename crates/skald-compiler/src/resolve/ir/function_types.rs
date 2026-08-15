//! Canonical closed function-type identities produced during resolution.

use crate::{id_table::DenseIdTable, identity::FunctionTypeId, source::Span};

use super::ResolvedType;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ResolvedFunctionTypeParameterMode {
    Value,
    ReadOnlyAlias,
    MutableAlias,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedFunctionTypeParameter {
    pub mode: ResolvedFunctionTypeParameterMode,
    pub type_syntax: ResolvedType,
    pub span: Span,
}

/// One canonical function signature.
///
/// The first source occurrence supplies the retained spans. Identity itself is
/// based solely on parameter modes, resolved child types, and the result type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedFunctionType {
    pub id: FunctionTypeId,
    pub parameters: Vec<ResolvedFunctionTypeParameter>,
    pub result: ResolvedType,
    pub span: Span,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ResolvedFunctionTypeTable {
    entries: DenseIdTable<FunctionTypeId, ResolvedFunctionType>,
}

impl ResolvedFunctionTypeTable {
    pub(crate) fn new(entries: Vec<ResolvedFunctionType>) -> Self {
        Self {
            entries: DenseIdTable::new(entries, |entry| entry.id),
        }
    }

    pub fn get(&self, id: FunctionTypeId) -> Option<&ResolvedFunctionType> {
        self.entries.get(id, |entry| entry.id)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &ResolvedFunctionType> {
        self.entries.iter()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
