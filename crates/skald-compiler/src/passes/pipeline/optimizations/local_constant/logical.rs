//! Exact transfer semantics for verified short-circuit records.

use crate::mir::MirLogicalOperation;

use super::super::primitive_evaluation::PrimitiveConstant;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum LogicalTransferSelection {
    Short(PrimitiveConstant),
    Right,
}

pub(super) fn select_logical_path(
    operation: MirLogicalOperation,
    left: PrimitiveConstant,
) -> Option<LogicalTransferSelection> {
    let PrimitiveConstant::Bool(left) = left else {
        return None;
    };
    Some(match (operation, left) {
        (MirLogicalOperation::And, false) => {
            LogicalTransferSelection::Short(PrimitiveConstant::Bool(false))
        }
        (MirLogicalOperation::And, true) => LogicalTransferSelection::Right,
        (MirLogicalOperation::Or, true) => {
            LogicalTransferSelection::Short(PrimitiveConstant::Bool(true))
        }
        (MirLogicalOperation::Or, false) => LogicalTransferSelection::Right,
    })
}
