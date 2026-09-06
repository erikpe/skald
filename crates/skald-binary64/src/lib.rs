//! Target-independent IEEE 754 binary64 values for the Skald compiler.
//!
//! [`Binary64`] stores the exact interchange bits of a binary64 value. It is a
//! compiler-only abstraction: generated programs and the Skald runtime do not
//! link this crate.
//!
//! The facade deliberately does not implement Rust's comparison traits. Bitwise
//! identity and IEEE numeric comparisons answer different questions, so callers
//! must select an explicit operation instead.
//!
//! ```compile_fail
//! use skald_binary64::Binary64;
//!
//! let positive_zero = Binary64::from_bits(0);
//! let negative_zero = Binary64::from_bits(0x8000_0000_0000_0000);
//! let _ = positive_zero == negative_zero;
//! ```
//!
//! ```compile_fail
//! use skald_binary64::Binary64;
//!
//! let one = Binary64::from_bits(0x3ff0_0000_0000_0000);
//! let two = Binary64::from_bits(0x4000_0000_0000_0000);
//! let _ = one < two;
//! ```

mod value;

pub use value::Binary64;

#[cfg(test)]
mod tests;
