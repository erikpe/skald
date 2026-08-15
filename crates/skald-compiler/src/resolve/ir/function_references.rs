//! Exact callable references and deterministic address-taken metadata.

use crate::{
    identity::{CallableId, FunctionTypeId},
    source::Span,
};

/// One source expression that forms a capture-free function value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolvedFunctionReferenceExpr {
    pub target: CallableId,
    pub function_type: FunctionTypeId,
    pub span: Span,
}

/// One exact callable whose address is formed anywhere in the resolved
/// program. The retained span is the first deterministic formation site.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolvedAddressTakenCallable {
    pub target: CallableId,
    pub function_type: FunctionTypeId,
    pub first_reference_span: Span,
}

/// A target-ordered set of exact address-taken callables.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ResolvedAddressTakenCallableTable {
    entries: Vec<ResolvedAddressTakenCallable>,
}

impl ResolvedAddressTakenCallableTable {
    pub fn get(&self, target: CallableId) -> Option<&ResolvedAddressTakenCallable> {
        self.entries
            .binary_search_by_key(&target, |entry| entry.target)
            .ok()
            .map(|index| &self.entries[index])
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &ResolvedAddressTakenCallable> {
        self.entries.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(crate) fn record(
        &mut self,
        target: CallableId,
        function_type: FunctionTypeId,
        first_reference_span: Span,
    ) {
        debug_assert!(matches!(
            target,
            CallableId::Function(_) | CallableId::Method(_)
        ));
        match self
            .entries
            .binary_search_by_key(&target, |entry| entry.target)
        {
            Ok(index) => debug_assert_eq!(self.entries[index].function_type, function_type),
            Err(index) => self.entries.insert(
                index,
                ResolvedAddressTakenCallable {
                    target,
                    function_type,
                    first_reference_span,
                },
            ),
        }
    }
}

#[cfg(test)]
mod tests;
