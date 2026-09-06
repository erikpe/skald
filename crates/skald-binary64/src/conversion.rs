use rustc_apfloat::{ieee::Double, Float, Round, Status, StatusAnd};

use crate::{
    apfloat::{apfloat_to_binary64, binary64_to_apfloat},
    Binary64,
};

const ROUND_TO_NEAREST: Round = Round::NearestTiesToEven;
const TRUNCATE: Round = Round::TowardZero;
const ONE_BITS: u64 = 0x3ff0_0000_0000_0000;

/// The result of converting a binary64 value to an integer type.
#[must_use]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntegerConversion<T> {
    /// The finite, truncated value is representable by the requested type.
    Value(T),
    /// The source is non-finite or its truncated value is outside the target range.
    OutOfRange,
}

impl Binary64 {
    /// Converts an `i64` to binary64 using round-to-nearest, ties-to-even.
    #[must_use]
    pub fn from_i64(value: i64) -> Self {
        finish_integer_to_binary64(Double::from_i128_r(i128::from(value), ROUND_TO_NEAREST))
    }

    /// Converts a `u64` to binary64 using round-to-nearest, ties-to-even.
    #[must_use]
    pub fn from_u64(value: u64) -> Self {
        finish_integer_to_binary64(Double::from_u128_r(u128::from(value), ROUND_TO_NEAREST))
    }

    /// Converts a `u8` to its exact binary64 representation.
    #[must_use]
    pub fn from_u8(value: u8) -> Self {
        finish_integer_to_binary64(Double::from_u128_r(u128::from(value), ROUND_TO_NEAREST))
    }

    /// Converts `false` to positive zero and `true` to exact binary64 one.
    #[must_use]
    pub const fn from_bool(value: bool) -> Self {
        Self::from_bits(if value { ONE_BITS } else { 0 })
    }

    /// Returns false for either zero and true for every other bit pattern.
    #[must_use]
    pub const fn to_bool(self) -> bool {
        !self.is_zero()
    }

    /// Truncates toward zero and converts to `i64` when the result is in range.
    pub fn truncating_to_i64(self) -> IntegerConversion<i64> {
        if !self.is_finite() {
            return IntegerConversion::OutOfRange;
        }

        let mut is_exact = false;
        let result = binary64_to_apfloat(self).to_i128_r(64, TRUNCATE, &mut is_exact);
        finish_binary64_to_integer(result, |value| {
            i64::try_from(value).expect("successful 64-bit signed APFloat conversion exceeded i64")
        })
    }

    /// Truncates toward zero and converts to `u64` when the result is in range.
    pub fn truncating_to_u64(self) -> IntegerConversion<u64> {
        if !self.is_finite() {
            return IntegerConversion::OutOfRange;
        }

        let mut is_exact = false;
        let result = binary64_to_apfloat(self).to_u128_r(64, TRUNCATE, &mut is_exact);
        finish_binary64_to_integer(result, |value| {
            u64::try_from(value)
                .expect("successful 64-bit unsigned APFloat conversion exceeded u64")
        })
    }

    /// Truncates toward zero and converts to `u8` when the result is in range.
    pub fn truncating_to_u8(self) -> IntegerConversion<u8> {
        if !self.is_finite() {
            return IntegerConversion::OutOfRange;
        }

        let mut is_exact = false;
        let result = binary64_to_apfloat(self).to_u128_r(8, TRUNCATE, &mut is_exact);
        finish_binary64_to_integer(result, |value| {
            u8::try_from(value).expect("successful 8-bit unsigned APFloat conversion exceeded u8")
        })
    }
}

fn finish_integer_to_binary64(result: StatusAnd<Double>) -> Binary64 {
    assert!(
        result.status == Status::OK || result.status == Status::INEXACT,
        "rustc_apfloat returned an unexpected integer-to-binary64 status: {:?}",
        result.status
    );
    apfloat_to_binary64(result.value)
}

fn finish_binary64_to_integer<T, U>(
    result: StatusAnd<T>,
    narrow: impl FnOnce(T) -> U,
) -> IntegerConversion<U> {
    // INEXACT means truncation discarded a fractional part; it is still a
    // successful Skald conversion when the truncated integer is in range.
    if result.status == Status::OK || result.status == Status::INEXACT {
        IntegerConversion::Value(narrow(result.value))
    } else if result.status == Status::INVALID_OP {
        IntegerConversion::OutOfRange
    } else {
        panic!(
            "rustc_apfloat returned an unexpected binary64-to-integer status: {:?}",
            result.status
        );
    }
}
