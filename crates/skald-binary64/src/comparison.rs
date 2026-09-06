use core::cmp::Ordering;

use rustc_apfloat::{ieee::Double, Float};

use crate::Binary64;

/// The result of an IEEE binary64 numeric comparison.
///
/// `Unordered` means at least one operand is a NaN. Unlike
/// [`Binary64::same_bits`], numeric comparison considers both zeroes equal.
#[must_use]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Binary64Comparison {
    /// The left operand is numerically less than the right operand.
    Less,
    /// The operands are numerically equal.
    Equal,
    /// The left operand is numerically greater than the right operand.
    Greater,
    /// At least one operand is a NaN.
    Unordered,
}

impl Binary64 {
    /// Compares two values using IEEE binary64 numeric comparison semantics.
    pub fn compare(self, other: Self) -> Binary64Comparison {
        let left = Double::from_bits(u128::from(self.to_bits()));
        let right = Double::from_bits(u128::from(other.to_bits()));

        match left.partial_cmp(&right) {
            Some(Ordering::Less) => Binary64Comparison::Less,
            Some(Ordering::Equal) => Binary64Comparison::Equal,
            Some(Ordering::Greater) => Binary64Comparison::Greater,
            None => Binary64Comparison::Unordered,
        }
    }
}
