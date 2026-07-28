//! String-literal validation and decoding.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum StringLiteralError {
    UnknownEscape,
    IncompleteEscape,
    IncompleteHexEscape,
    NonPrintableAscii,
    NonAscii,
    UnescapedNewline,
    Unterminated,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct StringLiteralScan {
    pub byte_len: usize,
    pub error: Option<StringLiteralError>,
}

/// Scans one literal beginning at a double quote.
///
/// Malformed content is consumed through its closing quote where possible so
/// it remains one invalid token. A physical newline is a hard recovery
/// boundary and is left for ordinary trivia handling.
pub(super) fn scan_string_literal(text: &str) -> StringLiteralScan {
    debug_assert!(text.starts_with('"'));
    let bytes = text.as_bytes();
    let mut index = 1;
    let mut error = None;

    while index < bytes.len() {
        match bytes[index] {
            b'"' => {
                return StringLiteralScan {
                    byte_len: index + 1,
                    error,
                };
            }
            b'\n' | b'\r' => {
                return StringLiteralScan {
                    byte_len: index,
                    error: Some(StringLiteralError::UnescapedNewline),
                };
            }
            b'\\' => {
                index += 1;
                if index == bytes.len() {
                    return StringLiteralScan {
                        byte_len: index,
                        error: Some(StringLiteralError::IncompleteEscape),
                    };
                }
                match bytes[index] {
                    b'"' | b'\\' | b'n' | b'r' | b't' | b'0' => index += 1,
                    b'x' => {
                        if index + 2 >= bytes.len()
                            || !bytes[index + 1].is_ascii_hexdigit()
                            || !bytes[index + 2].is_ascii_hexdigit()
                        {
                            error.get_or_insert(StringLiteralError::IncompleteHexEscape);
                            index += 1;
                        } else {
                            index += 3;
                        }
                    }
                    _ => {
                        error.get_or_insert(StringLiteralError::UnknownEscape);
                        index += 1;
                    }
                }
            }
            byte if !byte.is_ascii() => {
                error.get_or_insert(StringLiteralError::NonAscii);
                index += 1;
                while index < bytes.len() && !bytes[index].is_ascii() {
                    index += 1;
                }
            }
            0x20..=0x7e => index += 1,
            _ => {
                error.get_or_insert(StringLiteralError::NonPrintableAscii);
                index += 1;
            }
        }
    }

    StringLiteralScan {
        byte_len: bytes.len(),
        error: Some(error.unwrap_or(StringLiteralError::Unterminated)),
    }
}

/// Decodes a token already validated by [`scan_string_literal`].
pub(crate) fn decode_string_literal(lexeme: &str) -> Vec<u8> {
    debug_assert_eq!(
        scan_string_literal(lexeme).error,
        None,
        "only validated string tokens may be decoded"
    );
    let payload = &lexeme.as_bytes()[1..lexeme.len() - 1];
    let mut decoded = Vec::with_capacity(payload.len());
    let mut index = 0;
    while index < payload.len() {
        if payload[index] != b'\\' {
            decoded.push(payload[index]);
            index += 1;
            continue;
        }

        index += 1;
        match payload[index] {
            b'"' => decoded.push(b'"'),
            b'\\' => decoded.push(b'\\'),
            b'n' => decoded.push(b'\n'),
            b'r' => decoded.push(b'\r'),
            b't' => decoded.push(b'\t'),
            b'0' => decoded.push(b'\0'),
            b'x' => {
                decoded.push((hex_value(payload[index + 1]) << 4) | hex_value(payload[index + 2]));
                index += 2;
            }
            _ => unreachable!("validated string token has a known escape"),
        }
        index += 1;
    }
    decoded
}

fn hex_value(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        b'A'..=b'F' => byte - b'A' + 10,
        _ => unreachable!("validated hexadecimal digit"),
    }
}
