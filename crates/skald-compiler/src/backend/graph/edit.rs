//! Explicit partial ID mappings; an omitted ID never silently maps to itself.
use crate::backend::plan::PlanError;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum EditError {
    Context(PlanError),
    InvalidLocation,
    DuplicateId,
    MissingId,
    StaleSnapshot,
    TypeMismatch,
    DefinitionConflict,
}
impl From<PlanError> for EditError {
    fn from(error: PlanError) -> Self {
        Self::Context(error)
    }
}
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct IdMap<I> {
    pairs: BTreeMap<I, I>,
}
#[cfg_attr(not(test), allow(dead_code))]
impl<I: Copy + Ord> IdMap<I> {
    pub(super) fn new(pairs: BTreeMap<I, I>) -> Self {
        Self { pairs }
    }
    pub(in crate::backend) fn get(&self, id: I) -> Result<I, EditError> {
        self.pairs.get(&id).copied().ok_or(EditError::MissingId)
    }
}
