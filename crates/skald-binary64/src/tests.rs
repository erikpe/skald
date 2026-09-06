use rustc_apfloat::{ieee::Double, Float};

use super::{Binary64, Binary64Comparison};

const POSITIVE_ZERO: u64 = 0x0000_0000_0000_0000;
const NEGATIVE_ZERO: u64 = 0x8000_0000_0000_0000;
const ONE: u64 = 0x3ff0_0000_0000_0000;
const NEGATIVE_ONE: u64 = 0xbff0_0000_0000_0000;
const TWO: u64 = 0x4000_0000_0000_0000;
const POSITIVE_INFINITY: u64 = 0x7ff0_0000_0000_0000;
const NEGATIVE_INFINITY: u64 = 0xfff0_0000_0000_0000;
const CANONICAL_QUIET_NAN: u64 = 0x7ff8_0000_0000_0000;

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

#[test]
fn arithmetic_produces_exact_finite_results_and_cancellation() {
    assert_result(
        Binary64::from_bits(ONE).add(Binary64::from_bits(TWO)),
        0x4008_0000_0000_0000,
    );
    assert_result(
        Binary64::from_bits(0x4008_0000_0000_0000).subtract(Binary64::from_bits(ONE)),
        TWO,
    );
    assert_result(
        Binary64::from_bits(0x3ff8_0000_0000_0000).multiply(Binary64::from_bits(TWO)),
        0x4008_0000_0000_0000,
    );
    assert_result(
        Binary64::from_bits(0x4008_0000_0000_0000).divide(Binary64::from_bits(TWO)),
        0x3ff8_0000_0000_0000,
    );
    assert_result(
        Binary64::from_bits(ONE).add(Binary64::from_bits(NEGATIVE_ONE)),
        POSITIVE_ZERO,
    );
    assert_result(
        Binary64::from_bits(NEGATIVE_ZERO).add(Binary64::from_bits(NEGATIVE_ZERO)),
        NEGATIVE_ZERO,
    );
}

#[test]
fn arithmetic_rounds_to_nearest_with_ties_to_even() {
    let half_unit_at_one = Binary64::from_bits(0x3ca0_0000_0000_0000);

    assert_result(Binary64::from_bits(ONE).add(half_unit_at_one), ONE);
    assert_result(
        Binary64::from_bits(0x3ff0_0000_0000_0001).add(half_unit_at_one),
        0x3ff0_0000_0000_0002,
    );
    assert_result(
        Binary64::from_bits(ONE).divide(Binary64::from_bits(0x4008_0000_0000_0000)),
        0x3fd5_5555_5555_5555,
    );
}

#[test]
fn arithmetic_preserves_gradual_underflow_and_zero_signs() {
    let smallest_subnormal = Binary64::from_bits(1);
    let largest_subnormal = Binary64::from_bits(0x000f_ffff_ffff_ffff);
    let smallest_normal = Binary64::from_bits(0x0010_0000_0000_0000);

    assert_result(smallest_normal.subtract(largest_subnormal), 1);
    assert_result(
        smallest_normal.divide(Binary64::from_bits(TWO)),
        0x0008_0000_0000_0000,
    );
    assert_result(
        smallest_subnormal.divide(Binary64::from_bits(TWO)),
        POSITIVE_ZERO,
    );
    assert_result(
        smallest_subnormal.multiply(Binary64::from_bits(0x3ff8_0000_0000_0000)),
        2,
    );
    assert_result(
        Binary64::from_bits(0x8000_0000_0000_0001).divide(Binary64::from_bits(TWO)),
        NEGATIVE_ZERO,
    );
    assert_result(
        Binary64::from_bits(NEGATIVE_ZERO).divide(Binary64::from_bits(TWO)),
        NEGATIVE_ZERO,
    );
}

#[test]
fn arithmetic_returns_infinities_for_overflow_and_zero_divisors() {
    let largest_finite = Binary64::from_bits(0x7fef_ffff_ffff_ffff);
    let negative_largest_finite = Binary64::from_bits(0xffef_ffff_ffff_ffff);

    assert_result(
        largest_finite.multiply(Binary64::from_bits(TWO)),
        POSITIVE_INFINITY,
    );
    assert_result(
        negative_largest_finite.multiply(Binary64::from_bits(TWO)),
        NEGATIVE_INFINITY,
    );
    assert_result(
        Binary64::from_bits(ONE).divide(Binary64::from_bits(POSITIVE_ZERO)),
        POSITIVE_INFINITY,
    );
    assert_result(
        Binary64::from_bits(ONE).divide(Binary64::from_bits(NEGATIVE_ZERO)),
        NEGATIVE_INFINITY,
    );
    assert_result(
        Binary64::from_bits(NEGATIVE_ONE).divide(Binary64::from_bits(NEGATIVE_ZERO)),
        POSITIVE_INFINITY,
    );
    assert_result(
        Binary64::from_bits(POSITIVE_INFINITY).add(Binary64::from_bits(ONE)),
        POSITIVE_INFINITY,
    );
    assert_result(
        Binary64::from_bits(NEGATIVE_INFINITY).multiply(Binary64::from_bits(NEGATIVE_ONE)),
        POSITIVE_INFINITY,
    );
    assert_result(
        Binary64::from_bits(ONE).divide(Binary64::from_bits(POSITIVE_INFINITY)),
        POSITIVE_ZERO,
    );
    assert_result(
        Binary64::from_bits(NEGATIVE_ONE).divide(Binary64::from_bits(POSITIVE_INFINITY)),
        NEGATIVE_ZERO,
    );
}

#[test]
fn invalid_arithmetic_returns_the_calculated_nan() {
    let zero = Binary64::from_bits(POSITIVE_ZERO);
    let infinity = Binary64::from_bits(POSITIVE_INFINITY);

    assert_result(zero.divide(zero), CANONICAL_QUIET_NAN);
    assert_result(infinity.subtract(infinity), CANONICAL_QUIET_NAN);
    assert_result(infinity.multiply(zero), CANONICAL_QUIET_NAN);
    assert_result(infinity.divide(infinity), CANONICAL_QUIET_NAN);
}

#[test]
fn arithmetic_preserves_and_quiets_nan_payloads() {
    let quiet_nan = Binary64::from_bits(0x7ff8_0000_0000_0042);
    let other_quiet_nan = Binary64::from_bits(0xfff8_0000_0000_1234);
    let signaling_nan = Binary64::from_bits(0x7ff0_0000_0000_0042);
    let negative_signaling_nan = Binary64::from_bits(0xfff0_0000_0000_1234);
    let one = Binary64::from_bits(ONE);

    assert_result(quiet_nan.add(one), 0x7ff8_0000_0000_0042);
    assert_result(one.divide(other_quiet_nan), 0xfff8_0000_0000_1234);
    assert_result(signaling_nan.multiply(one), 0x7ff8_0000_0000_0042);
    assert_result(negative_signaling_nan.subtract(one), 0xfff8_0000_0000_1234);
    assert_result(quiet_nan.add(other_quiet_nan), 0x7ff8_0000_0000_0042);
}

#[test]
fn numeric_comparison_covers_every_ordered_relation() {
    let cases = [
        (NEGATIVE_INFINITY, NEGATIVE_ONE, Binary64Comparison::Less),
        (NEGATIVE_ONE, NEGATIVE_ZERO, Binary64Comparison::Less),
        (NEGATIVE_ZERO, POSITIVE_ZERO, Binary64Comparison::Equal),
        (ONE, ONE, Binary64Comparison::Equal),
        (TWO, ONE, Binary64Comparison::Greater),
        (POSITIVE_INFINITY, TWO, Binary64Comparison::Greater),
        (
            POSITIVE_INFINITY,
            POSITIVE_INFINITY,
            Binary64Comparison::Equal,
        ),
    ];

    for (left, right, expected) in cases {
        assert_eq!(
            Binary64::from_bits(left).compare(Binary64::from_bits(right)),
            expected,
            "comparison of {left:#018x} and {right:#018x}"
        );
    }
}

#[test]
fn numeric_comparison_is_unordered_for_either_nan_operand() {
    let nan_patterns = [
        0x7ff8_0000_0000_0042,
        0xfff8_0000_0000_1234,
        0x7ff0_0000_0000_0042,
        0xfff0_0000_0000_1234,
    ];
    let one = Binary64::from_bits(ONE);

    for bits in nan_patterns {
        let nan = Binary64::from_bits(bits);
        assert_eq!(nan.compare(one), Binary64Comparison::Unordered);
        assert_eq!(one.compare(nan), Binary64Comparison::Unordered);
        assert_eq!(nan.compare(nan), Binary64Comparison::Unordered);
    }
}

fn assert_result(actual: Binary64, expected_bits: u64) {
    assert_eq!(actual.to_bits(), expected_bits);
}
