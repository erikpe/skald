//! Checked integer shift selection.

use super::{HirExpression, HirIntegerType, Type};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirCheckedShift {
    pub operation: HirShiftOperation,
    pub left: Box<HirExpression>,
    pub count: Box<HirExpression>,
}

impl HirCheckedShift {
    pub fn new(operation: HirShiftOperation, left: HirExpression, count: HirExpression) -> Self {
        assert_eq!(
            left.ty,
            operation.left_type(),
            "typed shift left operand must match its selected integer kind"
        );
        assert_eq!(
            count.ty,
            operation.count_type(),
            "typed shift count must have exact type `u64`"
        );
        Self {
            operation,
            left: Box::new(left),
            count: Box::new(count),
        }
    }

    pub fn validate(&self, result_type: Type) {
        assert_eq!(self.left.ty, self.operation.left_type());
        assert_eq!(self.count.ty, self.operation.count_type());
        assert_eq!(result_type, self.operation.result_type());
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct HirShiftOperation {
    pub direction: HirShiftDirection,
    pub left: HirIntegerType,
}

impl HirShiftOperation {
    pub const fn left_type(self) -> Type {
        self.left.operand_type()
    }

    pub const fn count_type(self) -> Type {
        Type::U64
    }

    pub const fn result_type(self) -> Type {
        self.left_type()
    }

    pub const fn width(self) -> u64 {
        match self.left {
            HirIntegerType::I64 | HirIntegerType::U64 => 64,
            HirIntegerType::U8 => 8,
        }
    }

    pub const fn right_shift_flavor(self) -> Option<HirRightShiftFlavor> {
        match (self.direction, self.left) {
            (HirShiftDirection::Left, _) => None,
            (HirShiftDirection::Right, HirIntegerType::I64) => {
                Some(HirRightShiftFlavor::Arithmetic)
            }
            (HirShiftDirection::Right, HirIntegerType::U64 | HirIntegerType::U8) => {
                Some(HirRightShiftFlavor::Logical)
            }
        }
    }

    pub const fn failure(self) -> HirShiftFailure {
        match self.direction {
            HirShiftDirection::Left | HirShiftDirection::Right => HirShiftFailure::CountOutOfRange,
        }
    }

    pub const fn mnemonic(self) -> &'static str {
        match (self.direction, self.left) {
            (HirShiftDirection::Left, _) => "shl",
            (HirShiftDirection::Right, HirIntegerType::I64) => "sar",
            (HirShiftDirection::Right, HirIntegerType::U64 | HirIntegerType::U8) => "shr",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum HirShiftDirection {
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum HirRightShiftFlavor {
    Arithmetic,
    Logical,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum HirShiftFailure {
    CountOutOfRange,
}
