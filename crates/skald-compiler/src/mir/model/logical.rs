//! Provenance for verified short-circuit control-flow shapes.

use crate::source::Span;

use super::{BlockId, PathConditionId, StorageId, ValueId};

/// One structured logical expression after it has become ordinary MIR control
/// flow.
///
/// The metadata does not execute and is not an eager logical operation. It
/// lets verification retain the selected HIR contract while backends continue
/// to consume only branches, stores, loads, and jumps.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirLogicalExpression {
    pub operation: MirLogicalOperation,
    pub condition: PathConditionId,
    pub result: StorageId,
    pub left_result: ValueId,
    pub split: BlockId,
    pub selection: BlockId,
    pub right_entry: BlockId,
    pub right_exit: BlockId,
    pub right_result: ValueId,
    pub short: BlockId,
    pub join: BlockId,
    pub selected_result: ValueId,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MirLogicalOperation {
    And,
    Or,
}

impl MirLogicalOperation {
    pub const fn fixed_short_result(self) -> bool {
        match self {
            Self::And => false,
            Self::Or => true,
        }
    }
}
