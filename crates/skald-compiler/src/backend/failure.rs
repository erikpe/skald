//! Exact runtime reporting messages; semantic/target mappings stay with their owners.

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(usize)]
pub(in crate::backend) enum FailureMessage {
    ObjectCastFailure,
    OptionalAccessFailure,
    OptionalGuardOverflow,
    OptionalPinnedMutation,
    ArrayAllocationFailure,
    ArrayIndexOutOfBounds,
    ArrayInvalidSliceBounds,
    ArraySliceLengthMismatch,
    OwnershipCountOverflow,
    ShiftCountOutOfRange,
    IntegerDivisionByZero,
    IntegerRemainderByZero,
    PrimitiveCastOutOfRange,
}

impl FailureMessage {
    pub(in crate::backend) const ALL: [Self; 13] = [
        Self::ObjectCastFailure,
        Self::OptionalAccessFailure,
        Self::OptionalGuardOverflow,
        Self::OptionalPinnedMutation,
        Self::ArrayAllocationFailure,
        Self::ArrayIndexOutOfBounds,
        Self::ArrayInvalidSliceBounds,
        Self::ArraySliceLengthMismatch,
        Self::OwnershipCountOverflow,
        Self::ShiftCountOutOfRange,
        Self::IntegerDivisionByZero,
        Self::IntegerRemainderByZero,
        Self::PrimitiveCastOutOfRange,
    ];

    pub(in crate::backend) const fn bytes(self) -> &'static [u8] {
        match self {
            Self::ObjectCastFailure => b"checked object cast failed",
            Self::OptionalAccessFailure => b"optional value is absent",
            Self::OptionalGuardOverflow => b"optional presence guard overflow",
            Self::OptionalPinnedMutation => b"cannot mutate a guarded optional value",
            Self::ArrayAllocationFailure => b"array allocation failed",
            Self::ArrayIndexOutOfBounds => b"array index out of bounds",
            Self::ArrayInvalidSliceBounds => b"array slice bounds are invalid",
            Self::ArraySliceLengthMismatch => b"array slice length mismatch",
            Self::OwnershipCountOverflow => b"ownership count overflow",
            Self::ShiftCountOutOfRange => b"shift count out of range",
            Self::IntegerDivisionByZero => b"integer division by zero",
            Self::IntegerRemainderByZero => b"integer remainder by zero",
            Self::PrimitiveCastOutOfRange => b"floating-point cast out of range",
        }
    }
}
