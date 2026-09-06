use rustc_apfloat::{ieee::Double, Float, Round, Status};

use crate::{apfloat::apfloat_to_binary64, Binary64};

const ROUNDING: Round = Round::NearestTiesToEven;

/// The result of converting a validated decimal floating-literal spelling.
#[must_use]
#[derive(Clone, Copy, Debug)]
pub enum DecimalConversion {
    /// The spelling rounded to a finite binary64 value.
    Finite(Binary64),
    /// The spelling rounded to positive infinity.
    Overflow,
    /// The spelling does not satisfy Skald's unsigned floating-literal grammar.
    Invalid,
}

/// Converts a Skald decimal floating-literal spelling to binary64.
///
/// The accepted grammar requires decimal digits followed by either a fraction
/// or an exponent. A sign belongs to the source unary-expression grammar and
/// is therefore rejected here.
pub fn parse_decimal(spelling: &str) -> DecimalConversion {
    if !is_valid_spelling(spelling) {
        return DecimalConversion::Invalid;
    }

    let result = match Double::from_str_r(spelling, ROUNDING) {
        Ok(result) => result,
        Err(_) => return DecimalConversion::Invalid,
    };

    let underflow = Status::UNDERFLOW | Status::INEXACT;
    let overflow = Status::OVERFLOW | Status::INEXACT;
    if result.status != Status::OK
        && result.status != Status::INEXACT
        && result.status != underflow
        && result.status != overflow
    {
        return DecimalConversion::Invalid;
    }

    let value = apfloat_to_binary64(result.value);
    if value.is_infinite() {
        DecimalConversion::Overflow
    } else if value.is_finite() {
        DecimalConversion::Finite(value)
    } else {
        DecimalConversion::Invalid
    }
}

fn is_valid_spelling(spelling: &str) -> bool {
    let bytes = spelling.as_bytes();
    let mut index = take_digits(bytes, 0);
    if index == 0 {
        return false;
    }

    let mut has_fraction = false;
    if bytes.get(index) == Some(&b'.') {
        has_fraction = true;
        let fraction_start = index + 1;
        index = take_digits(bytes, fraction_start);
        if index == fraction_start {
            return false;
        }
    }

    let mut has_exponent = false;
    if matches!(bytes.get(index), Some(b'e' | b'E')) {
        has_exponent = true;
        index += 1;
        if matches!(bytes.get(index), Some(b'+' | b'-')) {
            index += 1;
        }
        let exponent_start = index;
        index = take_digits(bytes, exponent_start);
        if index == exponent_start {
            return false;
        }
    }

    (has_fraction || has_exponent) && index == bytes.len()
}

fn take_digits(bytes: &[u8], mut index: usize) -> usize {
    while bytes.get(index).is_some_and(u8::is_ascii_digit) {
        index += 1;
    }
    index
}
