//! Single-quoted byte-literal validation and decoding.

use super::escape::decode_hexadecimal_byte;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ByteLiteralError {
    Empty,
    MultipleBytes,
    UnknownEscape,
    IncompleteEscape,
    IncompleteHexEscape,
    NonPrintableAscii,
    NonAscii,
    UnescapedNewline,
    Unterminated,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ByteLiteralScan {
    pub byte_len: usize,
    pub value: Option<u8>,
    pub error: Option<ByteLiteralError>,
}

/// Scans one literal beginning at a single quote.
///
/// Malformed content is consumed through its closing quote where possible. A
/// physical newline is a hard recovery boundary and remains available to the
/// lexer's ordinary trivia handling.
pub(super) fn scan_byte_literal(text: &str) -> ByteLiteralScan {
    debug_assert!(text.starts_with('\''));
    let bytes = text.as_bytes();
    let mut index = 1;
    let mut value = None;
    let mut error = None;

    while index < bytes.len() {
        match bytes[index] {
            b'\'' => {
                let error = error.or_else(|| value.is_none().then_some(ByteLiteralError::Empty));
                return ByteLiteralScan {
                    byte_len: index + 1,
                    value: if error.is_none() { value } else { None },
                    error,
                };
            }
            b'\n' | b'\r' => {
                return ByteLiteralScan {
                    byte_len: index,
                    value: None,
                    error: Some(ByteLiteralError::UnescapedNewline),
                };
            }
            b'\\' => {
                if matches!(bytes.get(index + 1), Some(b'\n' | b'\r')) {
                    return ByteLiteralScan {
                        byte_len: index + 1,
                        value: None,
                        error: Some(ByteLiteralError::UnescapedNewline),
                    };
                }
                let (decoded, next_index) = scan_escape(bytes, index);
                match decoded {
                    Ok(decoded) => record_value(decoded, &mut value, &mut error),
                    Err(escape_error) => {
                        error.get_or_insert(escape_error);
                    }
                }
                index = next_index;
            }
            byte if !byte.is_ascii() => {
                error.get_or_insert(ByteLiteralError::NonAscii);
                index += 1;
                while index < bytes.len() && !bytes[index].is_ascii() {
                    index += 1;
                }
            }
            byte @ 0x20..=0x7e => {
                record_value(byte, &mut value, &mut error);
                index += 1;
            }
            _ => {
                error.get_or_insert(ByteLiteralError::NonPrintableAscii);
                index += 1;
            }
        }
    }

    ByteLiteralScan {
        byte_len: bytes.len(),
        value: None,
        error: Some(error.unwrap_or(ByteLiteralError::Unterminated)),
    }
}

/// Decodes a token already validated by [`scan_byte_literal`].
pub(crate) fn decode_byte_literal(lexeme: &str) -> u8 {
    let scan = scan_byte_literal(lexeme);
    debug_assert_eq!(
        scan.error, None,
        "only validated byte tokens may be decoded"
    );
    scan.value.expect("validated byte token has one value")
}

fn scan_escape(bytes: &[u8], index: usize) -> (Result<u8, ByteLiteralError>, usize) {
    let Some(escape) = bytes.get(index + 1).copied() else {
        return (Err(ByteLiteralError::IncompleteEscape), bytes.len());
    };
    match escape {
        b'\'' => (Ok(b'\''), index + 2),
        b'"' => (Ok(b'"'), index + 2),
        b'\\' => (Ok(b'\\'), index + 2),
        b'n' => (Ok(b'\n'), index + 2),
        b'r' => (Ok(b'\r'), index + 2),
        b't' => (Ok(b'\t'), index + 2),
        b'0' => (Ok(b'\0'), index + 2),
        b'x' => {
            let high = bytes.get(index + 2).copied();
            let low = bytes.get(index + 3).copied();
            match (high, low) {
                (Some(high), Some(low)) if high.is_ascii_hexdigit() && low.is_ascii_hexdigit() => {
                    (Ok(decode_hexadecimal_byte(high, low)), index + 4)
                }
                _ => (Err(ByteLiteralError::IncompleteHexEscape), index + 2),
            }
        }
        _ => (Err(ByteLiteralError::UnknownEscape), index + 2),
    }
}

fn record_value(value: u8, decoded: &mut Option<u8>, error: &mut Option<ByteLiteralError>) {
    if decoded.replace(value).is_some() {
        error.get_or_insert(ByteLiteralError::MultipleBytes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_direct_bytes_and_every_supported_escape() {
        for (spelling, expected) in [
            ("' '", b' '),
            ("'~'", b'~'),
            ("'\"'", b'"'),
            ("'\\\''", b'\''),
            ("'\\\"'", b'"'),
            ("'\\\\'", b'\\'),
            ("'\\n'", b'\n'),
            ("'\\r'", b'\r'),
            ("'\\t'", b'\t'),
            ("'\\0'", b'\0'),
            ("'\\x00'", 0),
            ("'\\xAf'", 0xaf),
            ("'\\xff'", 0xff),
        ] {
            let scan = scan_byte_literal(spelling);
            assert_eq!(scan.byte_len, spelling.len(), "{spelling}");
            assert_eq!(scan.error, None, "{spelling}");
            assert_eq!(scan.value, Some(expected), "{spelling}");
            assert_eq!(decode_byte_literal(spelling), expected, "{spelling}");
        }
    }

    #[test]
    fn classifies_malformed_content_and_recovers_through_a_closing_quote() {
        for (spelling, expected) in [
            ("''", ByteLiteralError::Empty),
            ("'ab'", ByteLiteralError::MultipleBytes),
            ("'\\q'", ByteLiteralError::UnknownEscape),
            ("'\\x'", ByteLiteralError::IncompleteHexEscape),
            ("'\\x4'", ByteLiteralError::IncompleteHexEscape),
            ("'\\xgg'", ByteLiteralError::IncompleteHexEscape),
            ("'\t'", ByteLiteralError::NonPrintableAscii),
            ("'é'", ByteLiteralError::NonAscii),
        ] {
            let scan = scan_byte_literal(spelling);
            assert_eq!(scan.byte_len, spelling.len(), "{spelling}");
            assert_eq!(scan.value, None, "{spelling}");
            assert_eq!(scan.error, Some(expected), "{spelling}");
        }
    }

    #[test]
    fn newline_and_end_of_file_are_hard_recovery_boundaries() {
        assert_eq!(
            scan_byte_literal("'a\nnext"),
            ByteLiteralScan {
                byte_len: 2,
                value: None,
                error: Some(ByteLiteralError::UnescapedNewline),
            }
        );
        assert_eq!(
            scan_byte_literal("'\\\nnext"),
            ByteLiteralScan {
                byte_len: 2,
                value: None,
                error: Some(ByteLiteralError::UnescapedNewline),
            }
        );
        assert_eq!(
            scan_byte_literal("'a"),
            ByteLiteralScan {
                byte_len: 2,
                value: None,
                error: Some(ByteLiteralError::Unterminated),
            }
        );
        assert_eq!(
            scan_byte_literal("'\\"),
            ByteLiteralScan {
                byte_len: 2,
                value: None,
                error: Some(ByteLiteralError::IncompleteEscape),
            }
        );
    }
}
