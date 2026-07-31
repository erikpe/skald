//! Target-independent checked integer shifts.

use super::control_flow::MirTerminationReason;
use super::{MirIntegerType, MirType, StorageId};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct MirShiftOperation {
    pub direction: MirShiftDirection,
    pub left: MirIntegerType,
}

impl MirShiftOperation {
    pub const fn left_type(self) -> MirType {
        self.left.operand_type()
    }

    pub const fn count_type(self) -> MirType {
        MirType::U64
    }

    pub const fn result_type(self) -> MirType {
        self.left_type()
    }

    pub const fn width(self) -> u64 {
        match self.left {
            MirIntegerType::I64 | MirIntegerType::U64 => 64,
            MirIntegerType::U8 => 8,
        }
    }

    pub const fn right_shift_flavor(self) -> Option<MirRightShiftFlavor> {
        match (self.direction, self.left) {
            (MirShiftDirection::Left, _) => None,
            (MirShiftDirection::Right, MirIntegerType::I64) => {
                Some(MirRightShiftFlavor::Arithmetic)
            }
            (MirShiftDirection::Right, MirIntegerType::U64 | MirIntegerType::U8) => {
                Some(MirRightShiftFlavor::Logical)
            }
        }
    }

    pub const fn failure_reason(self) -> MirTerminationReason {
        MirTerminationReason::ShiftCountOutOfRange
    }

    pub const fn mnemonic(self) -> &'static str {
        match (self.direction, self.left) {
            (MirShiftDirection::Left, _) => "shl",
            (MirShiftDirection::Right, MirIntegerType::I64) => "sar",
            (MirShiftDirection::Right, MirIntegerType::U64 | MirIntegerType::U8) => "shr",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MirShiftDirection {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MirRightShiftFlavor {
    Arithmetic,
    Logical,
}

/// Exact scalar carriers participating in one checked shift diamond.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MirShiftCountCheck {
    pub operation: MirShiftOperation,
    pub left: StorageId,
    pub count: StorageId,
    pub result: StorageId,
}
