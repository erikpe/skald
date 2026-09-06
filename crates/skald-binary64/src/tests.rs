use rustc_apfloat::{ieee::Double, Float};

use super::Binary64;

#[test]
fn raw_bits_round_trip_for_edge_and_generated_patterns() {
    let edge_patterns = [
        0,
        1,
        0x000f_ffff_ffff_ffff,
        0x0010_0000_0000_0000,
        0x7fef_ffff_ffff_ffff,
        0x7ff0_0000_0000_0000,
        0x7ff0_0000_0000_0001,
        0x7ff8_0000_0000_0000,
        0x8000_0000_0000_0000,
        u64::MAX,
    ];

    for bits in edge_patterns {
        assert_eq!(Binary64::from_bits(bits).to_bits(), bits);
    }

    let mut bits = 0x6a09_e667_f3bc_c909;
    for _ in 0..16_384 {
        bits ^= bits << 13;
        bits ^= bits >> 7;
        bits ^= bits << 17;
        assert_eq!(Binary64::from_bits(bits).to_bits(), bits);
    }
}

#[test]
fn classification_covers_binary64_boundaries() {
    struct Case {
        bits: u64,
        zero: bool,
        finite: bool,
        infinite: bool,
        nan: bool,
        negative: bool,
    }

    let cases = [
        Case {
            bits: 0,
            zero: true,
            finite: true,
            infinite: false,
            nan: false,
            negative: false,
        },
        Case {
            bits: 0x8000_0000_0000_0000,
            zero: true,
            finite: true,
            infinite: false,
            nan: false,
            negative: true,
        },
        Case {
            bits: 1,
            zero: false,
            finite: true,
            infinite: false,
            nan: false,
            negative: false,
        },
        Case {
            bits: 0x000f_ffff_ffff_ffff,
            zero: false,
            finite: true,
            infinite: false,
            nan: false,
            negative: false,
        },
        Case {
            bits: 0x0010_0000_0000_0000,
            zero: false,
            finite: true,
            infinite: false,
            nan: false,
            negative: false,
        },
        Case {
            bits: 0x7fef_ffff_ffff_ffff,
            zero: false,
            finite: true,
            infinite: false,
            nan: false,
            negative: false,
        },
        Case {
            bits: 0x7ff0_0000_0000_0000,
            zero: false,
            finite: false,
            infinite: true,
            nan: false,
            negative: false,
        },
        Case {
            bits: 0xfff0_0000_0000_0000,
            zero: false,
            finite: false,
            infinite: true,
            nan: false,
            negative: true,
        },
        Case {
            bits: 0x7ff0_0000_0000_0001,
            zero: false,
            finite: false,
            infinite: false,
            nan: true,
            negative: false,
        },
        Case {
            bits: 0x7ff8_0000_0000_0042,
            zero: false,
            finite: false,
            infinite: false,
            nan: true,
            negative: false,
        },
        Case {
            bits: 0xfff8_0000_0000_0042,
            zero: false,
            finite: false,
            infinite: false,
            nan: true,
            negative: true,
        },
    ];

    for case in cases {
        let value = Binary64::from_bits(case.bits);
        assert_eq!(value.is_zero(), case.zero, "zero: {:#018x}", case.bits);
        assert_eq!(
            value.is_finite(),
            case.finite,
            "finite: {:#018x}",
            case.bits
        );
        assert_eq!(
            value.is_infinite(),
            case.infinite,
            "infinite: {:#018x}",
            case.bits
        );
        assert_eq!(value.is_nan(), case.nan, "NaN: {:#018x}", case.bits);
        assert_eq!(
            value.is_negative(),
            case.negative,
            "negative: {:#018x}",
            case.bits
        );
    }
}

#[test]
fn sign_negation_changes_only_the_sign_bit() {
    for bits in [
        0,
        1,
        0x3ff0_0000_0000_0000,
        0x7ff0_0000_0000_0000,
        0x7ff0_0000_0000_0001,
        0x7ff8_0000_0000_0042,
        u64::MAX,
    ] {
        let value = Binary64::from_bits(bits);
        let negated = value.negate();
        assert_eq!(negated.to_bits(), bits ^ 0x8000_0000_0000_0000);
        assert!(negated.negate().same_bits(value));
    }
}

#[test]
fn bitwise_identity_is_explicit() {
    let positive_zero = Binary64::from_bits(0);
    let negative_zero = Binary64::from_bits(0x8000_0000_0000_0000);
    let nan = Binary64::from_bits(0x7ff8_0000_0000_0042);

    assert!(positive_zero.same_bits(positive_zero));
    assert!(!positive_zero.same_bits(negative_zero));
    assert!(nan.same_bits(nan));
}

#[test]
fn selected_apfloat_release_preserves_binary64_interchange_bits() {
    for bits in [
        0,
        1,
        0x3ff0_0000_0000_0000,
        0x7ff0_0000_0000_0000,
        0x7ff0_0000_0000_0001,
        0x7ff8_0000_0000_0042,
        u64::MAX,
    ] {
        let value = Double::from_bits(u128::from(bits));
        assert_eq!(value.to_bits(), u128::from(bits));
    }
}
