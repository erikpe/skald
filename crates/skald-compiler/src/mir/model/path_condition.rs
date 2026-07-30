//! Explicit path-condition metadata for conditional MIR state.

use crate::source::Span;

use super::{BlockId, PathConditionId, StorageId};

/// One callable-owned boolean path decision whose alternatives reconverge.
///
/// The active and inactive predecessors must each write their canonical
/// boolean selection to `activation` immediately before jumping to `merge`.
/// State that differs between those predecessors can then remain explicitly
/// conditioned on this identity until ordinary control flow tests it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirPathCondition {
    pub id: PathConditionId,
    pub parent: Option<PathConditionId>,
    pub activation: StorageId,
    pub active_predecessor: BlockId,
    pub inactive_predecessor: BlockId,
    pub merge: BlockId,
    pub span: Span,
}
