//! Independent value oracles and deterministic inputs for primitive-cast tests.

use super::{MirIntegerType, MirPrimitiveType, PrimitiveValue};

pub(super) fn expected_pure_cast(source: PrimitiveValue, target: MirPrimitiveType) -> Option<u64> {
    match (source, target) {
        (PrimitiveValue::I64(value), MirPrimitiveType::I64 | MirPrimitiveType::U64) => {
            Some(value as u64)
        }
        (PrimitiveValue::I64(value), MirPrimitiveType::U8) => Some(u64::from(value as u8)),
        (PrimitiveValue::I64(value), MirPrimitiveType::F64) => {
            Some(signed_integer_to_f64_bits(value))
        }
        (PrimitiveValue::I64(value), MirPrimitiveType::Bool) => Some(u64::from(value != 0)),
        (PrimitiveValue::U64(value), MirPrimitiveType::I64 | MirPrimitiveType::U64) => Some(value),
        (PrimitiveValue::U64(value), MirPrimitiveType::U8) => Some(u64::from(value as u8)),
        (PrimitiveValue::U64(value), MirPrimitiveType::F64) => {
            Some(unsigned_integer_to_f64_bits(value))
        }
        (PrimitiveValue::U64(value), MirPrimitiveType::Bool) => Some(u64::from(value != 0)),
        (PrimitiveValue::U8(value), MirPrimitiveType::I64 | MirPrimitiveType::U64) => {
            Some(u64::from(value))
        }
        (PrimitiveValue::U8(value), MirPrimitiveType::U8) => Some(u64::from(value)),
        (PrimitiveValue::U8(value), MirPrimitiveType::F64) => {
            Some(unsigned_integer_to_f64_bits(u64::from(value)))
        }
        (PrimitiveValue::U8(value), MirPrimitiveType::Bool) => Some(u64::from(value != 0)),
        (PrimitiveValue::F64Bits(bits), MirPrimitiveType::F64) => Some(bits),
        (PrimitiveValue::F64Bits(bits), MirPrimitiveType::Bool) => {
            Some(u64::from(bits & 0x7fff_ffff_ffff_ffff != 0))
        }
        (PrimitiveValue::F64Bits(_), _) => None,
        (PrimitiveValue::Bool(value), MirPrimitiveType::F64) => {
            Some(if value { 1.0_f64.to_bits() } else { 0 })
        }
        (PrimitiveValue::Bool(value), _) => Some(u64::from(value)),
    }
}

pub(super) fn expected_checked_cast(bits: u64, target: MirIntegerType) -> Option<u64> {
    let negative = bits >> 63 != 0;
    let TruncatedMagnitude::Value(magnitude) = truncated_magnitude(bits)? else {
        return None;
    };

    match target {
        MirIntegerType::I64 if negative && magnitude <= 1_u128 << 63 => {
            Some(0_u64.wrapping_sub(magnitude as u64))
        }
        MirIntegerType::I64 if !negative && magnitude <= i64::MAX as u128 => Some(magnitude as u64),
        MirIntegerType::U64 if !negative && magnitude <= u64::MAX as u128 => Some(magnitude as u64),
        MirIntegerType::U8 if !negative && magnitude <= u8::MAX as u128 => Some(magnitude as u64),
        MirIntegerType::U64 | MirIntegerType::U8 if negative && magnitude == 0 => Some(0),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TruncatedMagnitude {
    Value(u128),
    TooLarge,
}

/// Returns `None` for a non-finite input and an exact integer magnitude when
/// the finite value's truncation fits the oracle's deliberately wider domain.
fn truncated_magnitude(bits: u64) -> Option<TruncatedMagnitude> {
    let raw_exponent = ((bits >> 52) & 0x7ff) as i32;
    if raw_exponent == 0x7ff {
        return None;
    }
    if raw_exponent == 0 {
        return Some(TruncatedMagnitude::Value(0));
    }

    let exponent = raw_exponent - 1023;
    if exponent < 0 {
        return Some(TruncatedMagnitude::Value(0));
    }

    let significand = u128::from((1_u64 << 52) | (bits & 0x000f_ffff_ffff_ffff));
    if exponent <= 52 {
        Some(TruncatedMagnitude::Value(significand >> (52 - exponent)))
    } else {
        Some(
            significand
                .checked_shl((exponent - 52) as u32)
                .map_or(TruncatedMagnitude::TooLarge, TruncatedMagnitude::Value),
        )
    }
}

pub(super) fn pure_cast_samples() -> Vec<PrimitiveValue> {
    let mut samples = vec![
        PrimitiveValue::I64(i64::MIN),
        PrimitiveValue::I64(i64::MIN + 1),
        PrimitiveValue::I64(-(1_i64 << 53) - 1),
        PrimitiveValue::I64(-257),
        PrimitiveValue::I64(-256),
        PrimitiveValue::I64(-1),
        PrimitiveValue::I64(0),
        PrimitiveValue::I64(1),
        PrimitiveValue::I64(255),
        PrimitiveValue::I64(256),
        PrimitiveValue::I64(257),
        PrimitiveValue::I64((1_i64 << 53) - 1),
        PrimitiveValue::I64(1_i64 << 53),
        PrimitiveValue::I64((1_i64 << 53) + 1),
        PrimitiveValue::I64(i64::MAX),
        PrimitiveValue::U64(0),
        PrimitiveValue::U64(1),
        PrimitiveValue::U64(255),
        PrimitiveValue::U64(256),
        PrimitiveValue::U64(257),
        PrimitiveValue::U64((1_u64 << 53) - 1),
        PrimitiveValue::U64(1_u64 << 53),
        PrimitiveValue::U64((1_u64 << 53) + 1),
        PrimitiveValue::U64((1_u64 << 63) - 1),
        PrimitiveValue::U64(1_u64 << 63),
        PrimitiveValue::U64(u64::MAX),
        PrimitiveValue::U8(0),
        PrimitiveValue::U8(1),
        PrimitiveValue::U8(254),
        PrimitiveValue::U8(255),
        PrimitiveValue::F64Bits(0),
        PrimitiveValue::F64Bits(1_u64 << 63),
        PrimitiveValue::F64Bits(1),
        PrimitiveValue::F64Bits((1_u64 << 63) | 1),
        PrimitiveValue::F64Bits(255.999_f64.to_bits()),
        PrimitiveValue::F64Bits(256.0_f64.to_bits()),
        PrimitiveValue::F64Bits(((1_u64 << 53) as f64).to_bits()),
        PrimitiveValue::F64Bits(f64::INFINITY.to_bits()),
        PrimitiveValue::F64Bits(f64::NEG_INFINITY.to_bits()),
        PrimitiveValue::F64Bits(0x7ff8_1234_5678_9abc),
        PrimitiveValue::F64Bits(0xfff0_0000_0000_0001),
        PrimitiveValue::Bool(false),
        PrimitiveValue::Bool(true),
    ];

    let mut random = 0x6a09_e667_f3bc_c909_u64;
    for _ in 0..16 {
        random = xorshift64(random);
        samples.extend([
            PrimitiveValue::I64(random as i64),
            PrimitiveValue::U64(random),
            PrimitiveValue::U8(random as u8),
            PrimitiveValue::F64Bits(random),
        ]);
    }
    samples
}

pub(super) fn checked_cast_samples() -> Vec<u64> {
    let mut samples = vec![
        0,
        1_u64 << 63,
        1,
        (1_u64 << 63) | 1,
        f64::INFINITY.to_bits(),
        f64::NEG_INFINITY.to_bits(),
        0x7ff8_1234_5678_9abc,
        0x7ff0_0000_0000_0001,
        0xfff8_0000_0000_0042,
        0xfff0_0000_0000_0001,
    ];
    for center in [
        1.0_f64.to_bits(),
        (-1.0_f64).to_bits(),
        256.0_f64.to_bits(),
        (-256.0_f64).to_bits(),
        ((1_u64 << 53) as f64).to_bits(),
        (-((1_u64 << 53) as f64)).to_bits(),
        ((1_u64 << 63) as f64).to_bits(),
        (-((1_u64 << 63) as f64)).to_bits(),
        ((1_u128 << 64) as f64).to_bits(),
        (-((1_u128 << 64) as f64)).to_bits(),
    ] {
        for offset in 0..=4 {
            samples.push(center.wrapping_sub(offset));
            samples.push(center.wrapping_add(offset));
        }
    }

    let mut random = 0xbb67_ae85_84ca_a73b_u64;
    for _ in 0..32 {
        random = xorshift64(random);
        samples.push(random);
    }
    samples.sort_unstable();
    samples.dedup();
    samples
}

pub(super) fn signed_integer_to_f64_bits(value: i64) -> u64 {
    let sign = if value.is_negative() { 1_u64 << 63 } else { 0 };
    sign | unsigned_integer_to_f64_bits(value.unsigned_abs())
}

pub(super) fn unsigned_integer_to_f64_bits(value: u64) -> u64 {
    if value == 0 {
        return 0;
    }

    const FRACTION_BITS: u32 = 52;
    const FRACTION_MASK: u64 = (1_u64 << FRACTION_BITS) - 1;
    const EXPONENT_BIAS: u32 = 1023;

    let mut exponent = u64::BITS - 1 - value.leading_zeros();
    let significand = if exponent <= FRACTION_BITS {
        value << (FRACTION_BITS - exponent)
    } else {
        let discarded_bits = exponent - FRACTION_BITS;
        let mut retained = value >> discarded_bits;
        let remainder = value & ((1_u64 << discarded_bits) - 1);
        let halfway = 1_u64 << (discarded_bits - 1);
        if remainder > halfway || (remainder == halfway && retained & 1 != 0) {
            retained += 1;
        }
        if retained == 1_u64 << (FRACTION_BITS + 1) {
            exponent += 1;
            retained >>= 1;
        }
        retained
    };

    (u64::from(exponent + EXPONENT_BIAS) << FRACTION_BITS) | (significand & FRACTION_MASK)
}

fn xorshift64(mut value: u64) -> u64 {
    value ^= value << 13;
    value ^= value >> 7;
    value ^ (value << 17)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_oracle_uses_post_truncation_ranges() {
        assert_eq!(
            expected_checked_cast((-0.5_f64).to_bits(), MirIntegerType::U64),
            Some(0)
        );
        assert_eq!(
            expected_checked_cast(255.999_f64.to_bits(), MirIntegerType::U8),
            Some(255)
        );
        assert_eq!(
            expected_checked_cast(256.0_f64.to_bits(), MirIntegerType::U8),
            None
        );
        assert_eq!(
            expected_checked_cast(((1_u64 << 63) as f64).to_bits(), MirIntegerType::I64),
            None
        );
        assert_eq!(
            expected_checked_cast(f64::NAN.to_bits(), MirIntegerType::I64),
            None
        );
    }

    #[test]
    fn integer_to_f64_oracle_has_known_ties_and_exponent_carries() {
        assert_eq!(unsigned_integer_to_f64_bits(1), 0x3ff0_0000_0000_0000);
        assert_eq!(
            unsigned_integer_to_f64_bits((1_u64 << 53) + 1),
            0x4340_0000_0000_0000
        );
        assert_eq!(
            unsigned_integer_to_f64_bits((1_u64 << 53) + 3),
            0x4340_0000_0000_0002
        );
        assert_eq!(
            unsigned_integer_to_f64_bits(u64::MAX),
            0x43f0_0000_0000_0000
        );
        assert_eq!(signed_integer_to_f64_bits(i64::MIN), 0xc3e0_0000_0000_0000);
        assert_eq!(signed_integer_to_f64_bits(i64::MAX), 0x43e0_0000_0000_0000);
    }
}
