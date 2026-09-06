use skald_binary64::{Binary64, IntegerConversion};

const POSITIVE_ZERO: u64 = 0x0000_0000_0000_0000;
const NEGATIVE_ZERO: u64 = 0x8000_0000_0000_0000;
const POSITIVE_INFINITY: u64 = 0x7ff0_0000_0000_0000;
const NEGATIVE_INFINITY: u64 = 0xfff0_0000_0000_0000;

#[test]
fn signed_integer_conversion_rounds_to_nearest_with_ties_to_even() {
    let cases = [
        (0, POSITIVE_ZERO),
        (1, 0x3ff0_0000_0000_0000),
        (-1, 0xbff0_0000_0000_0000),
        ((1_i64 << 53) - 1, 0x433f_ffff_ffff_ffff),
        (1_i64 << 53, 0x4340_0000_0000_0000),
        ((1_i64 << 53) + 1, 0x4340_0000_0000_0000),
        ((1_i64 << 53) + 2, 0x4340_0000_0000_0001),
        (-((1_i64 << 53) + 1), 0xc340_0000_0000_0000),
        (i64::MAX, 0x43e0_0000_0000_0000),
        (i64::MIN, 0xc3e0_0000_0000_0000),
    ];

    for (integer, expected_bits) in cases {
        assert_eq!(
            Binary64::from_i64(integer).to_bits(),
            expected_bits,
            "conversion from {integer}"
        );
    }
}

#[test]
fn unsigned_integer_conversion_covers_precision_boundaries() {
    let cases = [
        (0, POSITIVE_ZERO),
        (1, 0x3ff0_0000_0000_0000),
        ((1_u64 << 53) - 1, 0x433f_ffff_ffff_ffff),
        (1_u64 << 53, 0x4340_0000_0000_0000),
        ((1_u64 << 53) + 1, 0x4340_0000_0000_0000),
        ((1_u64 << 53) + 2, 0x4340_0000_0000_0001),
        (1_u64 << 63, 0x43e0_0000_0000_0000),
        (u64::MAX, 0x43f0_0000_0000_0000),
    ];

    for (integer, expected_bits) in cases {
        assert_eq!(
            Binary64::from_u64(integer).to_bits(),
            expected_bits,
            "conversion from {integer}"
        );
    }
}

#[test]
fn byte_and_boolean_conversion_is_exact() {
    let cases = [
        (0, POSITIVE_ZERO),
        (1, 0x3ff0_0000_0000_0000),
        (2, 0x4000_0000_0000_0000),
        (127, 0x405f_c000_0000_0000),
        (255, 0x406f_e000_0000_0000),
    ];

    for (integer, expected_bits) in cases {
        assert_eq!(Binary64::from_u8(integer).to_bits(), expected_bits);
    }

    assert_eq!(Binary64::from_bool(false).to_bits(), POSITIVE_ZERO);
    assert_eq!(Binary64::from_bool(true).to_bits(), 0x3ff0_0000_0000_0000);
}

#[test]
fn conversion_to_bool_is_false_only_for_both_zeroes() {
    for bits in [POSITIVE_ZERO, NEGATIVE_ZERO] {
        assert!(!Binary64::from_bits(bits).to_bool());
    }

    for bits in [
        1,
        0x8000_0000_0000_0001,
        0x3ff0_0000_0000_0000,
        POSITIVE_INFINITY,
        NEGATIVE_INFINITY,
        0x7ff8_0000_0000_0042,
        0xfff0_0000_0000_1234,
    ] {
        assert!(Binary64::from_bits(bits).to_bool(), "bits {bits:#018x}");
    }
}

#[test]
fn truncating_to_i64_covers_both_boundaries_and_fractions() {
    let cases = [
        (POSITIVE_ZERO, IntegerConversion::Value(0)),
        (NEGATIVE_ZERO, IntegerConversion::Value(0)),
        (0x3ffe_6666_6666_6666, IntegerConversion::Value(1)),
        (0xbffe_6666_6666_6666, IntegerConversion::Value(-1)),
        (0xc3e0_0000_0000_0001, IntegerConversion::OutOfRange),
        (0xc3e0_0000_0000_0000, IntegerConversion::Value(i64::MIN)),
        (
            0xc3df_ffff_ffff_ffff,
            IntegerConversion::Value(-9_223_372_036_854_774_784),
        ),
        (
            0x43df_ffff_ffff_ffff,
            IntegerConversion::Value(9_223_372_036_854_774_784),
        ),
        (0x43e0_0000_0000_0000, IntegerConversion::OutOfRange),
    ];

    for (bits, expected) in cases {
        assert_eq!(
            Binary64::from_bits(bits).truncating_to_i64(),
            expected,
            "conversion of {bits:#018x}"
        );
    }
}

#[test]
fn truncating_to_u64_accepts_negative_fractions_but_not_negative_integers() {
    let cases = [
        (0xbff0_0000_0000_0000, IntegerConversion::OutOfRange),
        (0xbfef_ffff_ffff_ffff, IntegerConversion::Value(0)),
        (0xbfe0_0000_0000_0000, IntegerConversion::Value(0)),
        (0x8000_0000_0000_0001, IntegerConversion::Value(0)),
        (NEGATIVE_ZERO, IntegerConversion::Value(0)),
        (POSITIVE_ZERO, IntegerConversion::Value(0)),
        (0x3ffe_6666_6666_6666, IntegerConversion::Value(1)),
        (
            0x43ef_ffff_ffff_ffff,
            IntegerConversion::Value(18_446_744_073_709_549_568),
        ),
        (0x43f0_0000_0000_0000, IntegerConversion::OutOfRange),
    ];

    for (bits, expected) in cases {
        assert_eq!(
            Binary64::from_bits(bits).truncating_to_u64(),
            expected,
            "conversion of {bits:#018x}"
        );
    }
}

#[test]
fn truncating_to_u8_covers_both_boundaries_and_fractions() {
    let cases = [
        (0xbff0_0000_0000_0000, IntegerConversion::OutOfRange),
        (0xbfef_ffff_ffff_ffff, IntegerConversion::Value(0)),
        (POSITIVE_ZERO, IntegerConversion::Value(0)),
        (0x406f_e000_0000_0000, IntegerConversion::Value(255)),
        (0x406f_ffff_ffff_ffff, IntegerConversion::Value(255)),
        (0x4070_0000_0000_0000, IntegerConversion::OutOfRange),
    ];

    for (bits, expected) in cases {
        assert_eq!(
            Binary64::from_bits(bits).truncating_to_u8(),
            expected,
            "conversion of {bits:#018x}"
        );
    }
}

#[test]
fn every_non_finite_value_is_out_of_integer_range() {
    for bits in [
        POSITIVE_INFINITY,
        NEGATIVE_INFINITY,
        0x7ff8_0000_0000_0042,
        0xfff8_0000_0000_1234,
        0x7ff0_0000_0000_0042,
        0xfff0_0000_0000_1234,
    ] {
        let value = Binary64::from_bits(bits);
        assert_eq!(value.truncating_to_i64(), IntegerConversion::OutOfRange);
        assert_eq!(value.truncating_to_u64(), IntegerConversion::OutOfRange);
        assert_eq!(value.truncating_to_u8(), IntegerConversion::OutOfRange);
    }
}
