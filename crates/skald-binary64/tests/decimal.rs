use skald_binary64::{parse_decimal, DecimalConversion};

#[test]
fn decimal_zero_and_ordinary_values_have_fixed_bits() {
    let cases = [
        ("0.0", 0x0000_0000_0000_0000),
        ("0e999", 0x0000_0000_0000_0000),
        ("1.0", 0x3ff0_0000_0000_0000),
        ("1e0", 0x3ff0_0000_0000_0000),
        (
            "3.1415926535897932384626433832795028841971",
            0x4009_21fb_5444_2d18,
        ),
    ];

    for (spelling, expected_bits) in cases {
        assert_finite(spelling, expected_bits);
    }
}

#[test]
fn decimal_extrema_and_subnormals_have_fixed_bits() {
    let cases = [
        ("1.7976931348623157e308", 0x7fef_ffff_ffff_ffff),
        ("2.2250738585072014e-308", 0x0010_0000_0000_0000),
        ("2.225073858507201e-308", 0x000f_ffff_ffff_ffff),
        ("4.9406564584124654e-324", 0x0000_0000_0000_0001),
        ("1e-4000", 0x0000_0000_0000_0000),
    ];

    for (spelling, expected_bits) in cases {
        assert_finite(spelling, expected_bits);
    }
}

#[test]
fn decimal_halfway_cases_round_to_an_even_significand() {
    assert_finite(
        "1.00000000000000011102230246251565404236316680908203125",
        0x3ff0_0000_0000_0000,
    );
    assert_finite(
        "1.000000000000000111022302462515654042363166809082031251",
        0x3ff0_0000_0000_0001,
    );
    assert_finite(
        "1.00000000000000033306690738754696212708950042724609375",
        0x3ff0_0000_0000_0002,
    );
}

#[test]
fn decimal_overflow_is_distinct_from_invalid_input() {
    for spelling in ["1.7976931348623159e308", "1e309", "9e999999999"] {
        assert!(matches!(
            parse_decimal(spelling),
            DecimalConversion::Overflow
        ));
    }
}

#[test]
fn malformed_or_non_source_spellings_are_invalid() {
    for spelling in [
        "", "0", "1", ".5", "1.", "+1.0", "-1.0", "1e", "1e+", "1e-", "1.0e", "1.0e+", "1.0.0",
        "1e0e0", "1_0.0", " 1.0", "1.0 ", "NaN", "sNaN", "Infinity", "inf", "0x1.0p0", "１.０",
    ] {
        assert!(
            matches!(parse_decimal(spelling), DecimalConversion::Invalid),
            "spelling {spelling:?}"
        );
    }
}

fn assert_finite(spelling: &str, expected_bits: u64) {
    let DecimalConversion::Finite(value) = parse_decimal(spelling) else {
        panic!("expected {spelling:?} to produce a finite value");
    };
    assert_eq!(value.to_bits(), expected_bits, "spelling {spelling:?}");
}
