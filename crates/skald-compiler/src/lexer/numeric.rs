//! Centralized recognition of numeric-looking source spellings.

use crate::literal::{IntegerRadix, NumericLiteralKind};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct NumericScan {
    pub byte_len: usize,
    pub kind: Option<NumericLiteralKind>,
}

/// Scans one complete numeric-looking token from the start of `source`.
///
/// A recognized kind is accepted by the current lexer. A `None` kind means
/// that the complete recovered spelling is malformed.
pub(super) fn scan_numeric_literal(source: &str) -> NumericScan {
    let bytes = source.as_bytes();
    assert!(bytes.first().is_some_and(u8::is_ascii_digit));

    if bytes.starts_with(b"0x") || bytes.starts_with(b"0X") {
        return scan_hexadecimal_integer(bytes);
    }

    let mut end = take_digits(bytes, 0);
    let mut kind = NumericLiteralKind::I64(IntegerRadix::Decimal);
    let mut well_formed = true;

    match bytes.get(end).copied() {
        Some(b'u') => {
            end += 1;
            if bytes.get(end) == Some(&b'8') {
                end += 1;
                kind = NumericLiteralKind::U8(IntegerRadix::Decimal);
            } else {
                kind = NumericLiteralKind::U64(IntegerRadix::Decimal);
            }
        }
        Some(b'.') if bytes.get(end + 1).is_some_and(u8::is_ascii_digit) => {
            kind = NumericLiteralKind::F64;
            end += 1;
            let fraction_start = end;
            end = take_digits(bytes, end);
            well_formed = end > fraction_start;
            if matches!(bytes.get(end), Some(b'e' | b'E')) {
                let exponent = scan_exponent(bytes, end);
                end = exponent.end;
                well_formed &= exponent.well_formed;
            }
        }
        Some(b'e' | b'E') => {
            kind = NumericLiteralKind::F64;
            let exponent = scan_exponent(bytes, end);
            end = exponent.end;
            well_formed = exponent.well_formed;
        }
        _ => {}
    }

    if !bytes.get(end..).is_some_and(|tail| tail.starts_with(b".."))
        && bytes.get(end).is_some_and(|byte| is_numeric_tail(*byte))
    {
        well_formed = false;
        end = take_numeric_tail(bytes, end);
    }

    NumericScan {
        byte_len: end,
        kind: well_formed.then_some(kind),
    }
}

fn scan_hexadecimal_integer(bytes: &[u8]) -> NumericScan {
    debug_assert!(bytes.starts_with(b"0x") || bytes.starts_with(b"0X"));
    let digits_start = 2;
    let mut end = take_hexadecimal_digits(bytes, digits_start);
    let mut kind = NumericLiteralKind::I64(IntegerRadix::Hexadecimal);
    let mut well_formed = end > digits_start;

    if bytes.get(end) == Some(&b'u') {
        end += 1;
        if bytes.get(end) == Some(&b'8') {
            end += 1;
            kind = NumericLiteralKind::U8(IntegerRadix::Hexadecimal);
        } else {
            kind = NumericLiteralKind::U64(IntegerRadix::Hexadecimal);
        }
    }

    if bytes.get(end).is_some_and(|byte| is_numeric_tail(*byte)) {
        well_formed = false;
        end = take_numeric_tail(bytes, end);
    }

    NumericScan {
        byte_len: end,
        kind: well_formed.then_some(kind),
    }
}

#[derive(Clone, Copy)]
struct ExponentScan {
    end: usize,
    well_formed: bool,
}

fn scan_exponent(bytes: &[u8], mut end: usize) -> ExponentScan {
    debug_assert!(matches!(bytes.get(end), Some(b'e' | b'E')));
    end += 1;
    if matches!(bytes.get(end), Some(b'+' | b'-')) {
        end += 1;
    }
    let digits_start = end;
    end = take_digits(bytes, end);
    ExponentScan {
        end,
        well_formed: end > digits_start,
    }
}

fn take_digits(bytes: &[u8], mut end: usize) -> usize {
    while bytes.get(end).is_some_and(u8::is_ascii_digit) {
        end += 1;
    }
    end
}

fn take_hexadecimal_digits(bytes: &[u8], mut end: usize) -> usize {
    while bytes.get(end).is_some_and(u8::is_ascii_hexdigit) {
        end += 1;
    }
    end
}

fn take_numeric_tail(bytes: &[u8], mut end: usize) -> usize {
    while bytes.get(end).is_some_and(|byte| is_numeric_tail(*byte)) {
        end += 1;
    }
    end
}

const fn is_numeric_tail(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'.')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scan(source: &str) -> (usize, Option<NumericLiteralKind>) {
        let scan = scan_numeric_literal(source);
        (scan.byte_len, scan.kind)
    }

    #[test]
    fn classifies_every_contracted_numeric_form_without_converting_values() {
        assert_eq!(
            scan("42;"),
            (2, Some(NumericLiteralKind::I64(IntegerRadix::Decimal)))
        );
        assert_eq!(
            scan("42u;"),
            (3, Some(NumericLiteralKind::U64(IntegerRadix::Decimal)))
        );
        assert_eq!(
            scan("42u8;"),
            (4, Some(NumericLiteralKind::U8(IntegerRadix::Decimal)))
        );
        assert_eq!(scan("1.5;"), (3, Some(NumericLiteralKind::F64)));
        assert_eq!(scan("6e-2;"), (4, Some(NumericLiteralKind::F64)));
        assert_eq!(
            scan("0x2a;"),
            (4, Some(NumericLiteralKind::I64(IntegerRadix::Hexadecimal)))
        );
        assert_eq!(
            scan("0X2Au;"),
            (5, Some(NumericLiteralKind::U64(IntegerRadix::Hexadecimal)))
        );
        assert_eq!(
            scan("0xffu8;"),
            (6, Some(NumericLiteralKind::U8(IntegerRadix::Hexadecimal)))
        );
        assert_eq!(
            scan("0x1e3;"),
            (5, Some(NumericLiteralKind::I64(IntegerRadix::Hexadecimal)))
        );
    }

    #[test]
    fn consumes_malformed_numeric_tails_as_one_spelling() {
        for spelling in [
            "12abc", "1_000", "1.", "1e+", "1.2.3", "42u64", "0x", "0xg", "0x1g", "0xffu64",
            "0x1U", "0x1U8", "0x1u9", "0x_ff", "0x1.0",
        ] {
            assert_eq!(scan(spelling), (spelling.len(), None), "{spelling}");
        }
    }

    #[test]
    fn stops_at_source_token_boundaries() {
        assert_eq!(
            scan("1+2"),
            (1, Some(NumericLiteralKind::I64(IntegerRadix::Decimal)))
        );
        assert_eq!(
            scan("1u-2"),
            (2, Some(NumericLiteralKind::U64(IntegerRadix::Decimal)))
        );
        assert_eq!(
            scan("1é"),
            (1, Some(NumericLiteralKind::I64(IntegerRadix::Decimal)))
        );
        assert_eq!(
            scan("0x1e+2"),
            (4, Some(NumericLiteralKind::I64(IntegerRadix::Hexadecimal)))
        );
        assert_eq!(
            scan("1..3"),
            (1, Some(NumericLiteralKind::I64(IntegerRadix::Decimal)))
        );
        assert_eq!(scan("1.5..3"), (3, Some(NumericLiteralKind::F64)));
    }
}
