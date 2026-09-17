//! Closed scalar descriptors and local schema checks, independent of execution IRs.

use super::BuildError;
use crate::backend::plan::ScalarType;

/// Binary64 constants are bits, never host-formatted floating values. Rust's
/// byte/bool payload types make noncanonical constants unrepresentable here.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum Constant {
    I64(i64),
    U64(u64),
    U8(u8),
    Bool(bool),
    F64(u64),
    Null(ScalarType),
}
#[cfg_attr(not(test), allow(dead_code))]
impl Constant {
    pub(in crate::backend) fn scalar_type(self) -> Result<ScalarType, BuildError> {
        Ok(match self {
            Self::I64(_) => ScalarType::I64,
            Self::U64(_) => ScalarType::U64,
            Self::U8(_) => ScalarType::U8,
            Self::Bool(_) => ScalarType::Bool,
            Self::F64(_) => ScalarType::F64,
            Self::Null(ty @ (ScalarType::DataAddress | ScalarType::CodeAddress(_))) => ty,
            Self::Null(_) => return Err(BuildError::InvalidScalar),
        })
    }
}

/// Integer arithmetic wraps at the operand width. Float arithmetic preserves
/// the binary64 contract; descriptors grant no reassociation permission.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum UnaryOperation {
    Negate,
    Complement,
    LogicalNot,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum BinaryOperation {
    Add,
    Subtract,
    Multiply,
    FloatDivide,
    And,
    Or,
    Xor,
}
/// Signed division is floor division; MIN / -1 yields MIN, with remainder 0.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum DivisionResult {
    Quotient,
    Remainder,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum ShiftDirection {
    Left,
    ArithmeticRight,
    LogicalRight,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum Conversion {
    Identity,
    IntegerBits,
    ToBoolean,
    FromBoolean,
    ToFloat,
    FloatBits,
    TruncateFloat,
    PointerBits,
}

#[cfg_attr(not(test), allow(dead_code))]
pub(super) const fn integer(ty: ScalarType) -> bool {
    matches!(ty, ScalarType::I64 | ScalarType::U64 | ScalarType::U8)
}
#[cfg_attr(not(test), allow(dead_code))]
pub(super) const fn integer_width(ty: ScalarType) -> Option<u8> {
    match ty {
        ScalarType::I64 | ScalarType::U64 => Some(64),
        ScalarType::U8 => Some(8),
        _ => None,
    }
}
#[cfg_attr(not(test), allow(dead_code))]
impl Conversion {
    pub(in crate::backend) fn accepts(self, from: ScalarType, to: ScalarType) -> bool {
        use ScalarType::*;
        match self {
            Self::Identity => from == to,
            Self::IntegerBits => integer(from) && integer(to),
            Self::ToBoolean => (integer(from) || from == F64) && to == Bool,
            Self::FromBoolean => from == Bool && integer(to),
            Self::ToFloat => (integer(from) || from == Bool) && to == F64,
            Self::FloatBits => matches!((from, to), (F64, U64) | (U64, F64)),
            Self::TruncateFloat => from == F64 && integer(to),
            Self::PointerBits => matches!(
                (from, to),
                (DataAddress | CodeAddress(_), U64) | (U64, DataAddress | CodeAddress(_))
            ),
        }
    }
}

/// Storage width is independent of the scalar carrier. Narrow scalars may use
/// an eight-byte carrier; arbitrary truncating stores are separate conversions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct MemoryRepresentation {
    pub scalar: ScalarType,
    pub bytes: usize,
    pub alignment: usize,
}
#[cfg_attr(not(test), allow(dead_code))]
impl MemoryRepresentation {
    pub(in crate::backend) fn check(self, pointer_bytes: usize) -> Result<(), BuildError> {
        let width_ok = match self.scalar {
            ScalarType::U8 | ScalarType::Bool => matches!(self.bytes, 1 | 8),
            ScalarType::I64 | ScalarType::U64 | ScalarType::F64 => self.bytes == 8,
            ScalarType::DataAddress | ScalarType::CodeAddress(_) => self.bytes == pointer_bytes,
        };
        if !width_ok || self.alignment == 0 || !self.alignment.is_power_of_two() {
            return Err(BuildError::InvalidMemory);
        }
        self.bytes
            .checked_add(self.alignment - 1)
            .ok_or(BuildError::SizeOverflow)?;
        Ok(())
    }
}

/// A static scaled-address multiplier, computed without host-size overflow.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct AddressStride {
    bytes: usize,
}
#[cfg_attr(not(test), allow(dead_code))]
impl AddressStride {
    pub(in crate::backend) fn new(
        element_bytes: usize,
        elements: usize,
    ) -> Result<Self, BuildError> {
        Ok(Self {
            bytes: element_bytes
                .checked_mul(elements)
                .ok_or(BuildError::SizeOverflow)?,
        })
    }
    pub(in crate::backend) const fn bytes(self) -> usize {
        self.bytes
    }
}
