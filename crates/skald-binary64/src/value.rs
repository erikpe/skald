const SIGN_MASK: u64 = 0x8000_0000_0000_0000;
const EXPONENT_MASK: u64 = 0x7ff0_0000_0000_0000;
const FRACTION_MASK: u64 = 0x000f_ffff_ffff_ffff;

/// The exact interchange bits of an IEEE 754 binary64 value.
///
/// This type intentionally exposes only explicit operations. In particular, it
/// does not implement Rust's comparison traits because bitwise identity and
/// IEEE numeric comparisons have different semantics for zeros and NaNs.
#[derive(Clone, Copy, Debug)]
pub struct Binary64 {
    bits: u64,
}

impl Binary64 {
    /// Creates a value from its exact IEEE 754 binary64 interchange bits.
    #[must_use]
    pub const fn from_bits(bits: u64) -> Self {
        Self { bits }
    }

    /// Returns the exact IEEE 754 binary64 interchange bits.
    #[must_use]
    pub const fn to_bits(self) -> u64 {
        self.bits
    }

    /// Returns whether both values have exactly the same interchange bits.
    ///
    /// Unlike IEEE numeric equality, this distinguishes positive and negative
    /// zero and compares NaN payloads and signs.
    #[must_use]
    pub const fn same_bits(self, other: Self) -> bool {
        self.bits == other.bits
    }

    /// Returns the value with its sign bit inverted.
    ///
    /// This preserves every exponent and fraction bit, including NaN payloads.
    #[must_use]
    pub const fn negate(self) -> Self {
        Self::from_bits(self.bits ^ SIGN_MASK)
    }

    /// Returns whether this value is positive or negative zero.
    #[must_use]
    pub const fn is_zero(self) -> bool {
        self.bits & !SIGN_MASK == 0
    }

    /// Returns whether this value is finite, including zero and subnormal values.
    #[must_use]
    pub const fn is_finite(self) -> bool {
        self.bits & EXPONENT_MASK != EXPONENT_MASK
    }

    /// Returns whether this value is positive or negative infinity.
    #[must_use]
    pub const fn is_infinite(self) -> bool {
        self.bits & EXPONENT_MASK == EXPONENT_MASK && self.bits & FRACTION_MASK == 0
    }

    /// Returns whether this value is a quiet or signaling NaN.
    #[must_use]
    pub const fn is_nan(self) -> bool {
        self.bits & EXPONENT_MASK == EXPONENT_MASK && self.bits & FRACTION_MASK != 0
    }

    /// Returns whether the sign bit is set.
    ///
    /// The result is defined for every bit pattern, including zeros and NaNs.
    #[must_use]
    pub const fn is_negative(self) -> bool {
        self.bits & SIGN_MASK != 0
    }
}
