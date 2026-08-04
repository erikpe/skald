//! Delimiter-independent helpers shared by byte-oriented literal decoders.

pub(super) fn decode_hexadecimal_byte(high: u8, low: u8) -> u8 {
    (hexadecimal_digit_value(high) << 4) | hexadecimal_digit_value(low)
}

fn hexadecimal_digit_value(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        b'A'..=b'F' => byte - b'A' + 10,
        _ => unreachable!("validated hexadecimal digit"),
    }
}
