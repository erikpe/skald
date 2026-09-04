use crate::mir::{
    MirIntegerDivisionKind, MirIntegerDivisionOperation, MirIntegerType, MirShiftDirection,
    MirShiftOperation, MirTerminationReason,
};

use super::{
    evaluate_integer_division, evaluate_shift, CheckedIntegerEvaluation, PrimitiveConstant,
};

fn division(
    kind: MirIntegerDivisionKind,
    operand: MirIntegerType,
    dividend: PrimitiveConstant,
    divisor: PrimitiveConstant,
) -> CheckedIntegerEvaluation {
    evaluate_integer_division(
        MirIntegerDivisionOperation { kind, operand },
        dividend,
        divisor,
    )
}

fn shift(
    direction: MirShiftDirection,
    left_type: MirIntegerType,
    left: PrimitiveConstant,
    count: PrimitiveConstant,
) -> CheckedIntegerEvaluation {
    evaluate_shift(
        MirShiftOperation {
            direction,
            left: left_type,
        },
        left,
        count,
    )
}

fn success(constant: PrimitiveConstant) -> CheckedIntegerEvaluation {
    CheckedIntegerEvaluation::Success(constant)
}

#[test]
fn division_matrix_preserves_exact_integer_types() {
    let cases = [
        (
            MirIntegerType::I64,
            PrimitiveConstant::I64(7),
            PrimitiveConstant::I64(3),
            PrimitiveConstant::I64(2),
            PrimitiveConstant::I64(1),
        ),
        (
            MirIntegerType::U64,
            PrimitiveConstant::U64(7),
            PrimitiveConstant::U64(3),
            PrimitiveConstant::U64(2),
            PrimitiveConstant::U64(1),
        ),
        (
            MirIntegerType::U8,
            PrimitiveConstant::U8(7),
            PrimitiveConstant::U8(3),
            PrimitiveConstant::U8(2),
            PrimitiveConstant::U8(1),
        ),
    ];

    for (operand, dividend, divisor, quotient, remainder) in cases {
        assert_eq!(
            division(MirIntegerDivisionKind::Quotient, operand, dividend, divisor),
            success(quotient)
        );
        assert_eq!(
            division(
                MirIntegerDivisionKind::Remainder,
                operand,
                dividend,
                divisor
            ),
            success(remainder)
        );
    }
}

#[test]
fn signed_division_floors_and_remainder_has_divisor_sign() {
    let cases = [
        (7, 3, 2, 1),
        (-7, 3, -3, 2),
        (7, -3, -3, -2),
        (-7, -3, 2, -1),
        (1, i64::MIN, -1, i64::MIN + 1),
        (-1, i64::MIN, 0, -1),
    ];

    for (dividend, divisor, quotient, remainder) in cases {
        assert_eq!(
            division(
                MirIntegerDivisionKind::Quotient,
                MirIntegerType::I64,
                PrimitiveConstant::I64(dividend),
                PrimitiveConstant::I64(divisor),
            ),
            success(PrimitiveConstant::I64(quotient))
        );
        assert_eq!(
            division(
                MirIntegerDivisionKind::Remainder,
                MirIntegerType::I64,
                PrimitiveConstant::I64(dividend),
                PrimitiveConstant::I64(divisor),
            ),
            success(PrimitiveConstant::I64(remainder))
        );
        assert_eq!(
            dividend,
            quotient.wrapping_mul(divisor).wrapping_add(remainder)
        );
        assert!(remainder == 0 || (remainder < 0) == (divisor < 0));
    }
}

#[test]
fn signed_minimum_pair_is_defined_without_host_overflow() {
    assert_eq!(
        division(
            MirIntegerDivisionKind::Quotient,
            MirIntegerType::I64,
            PrimitiveConstant::I64(i64::MIN),
            PrimitiveConstant::I64(-1),
        ),
        success(PrimitiveConstant::I64(i64::MIN))
    );
    assert_eq!(
        division(
            MirIntegerDivisionKind::Remainder,
            MirIntegerType::I64,
            PrimitiveConstant::I64(i64::MIN),
            PrimitiveConstant::I64(-1),
        ),
        success(PrimitiveConstant::I64(0))
    );
}

#[test]
fn signed_extrema_match_mathematical_floor_division() {
    let values = [
        i64::MIN,
        i64::MIN + 1,
        -7,
        -3,
        -1,
        0,
        1,
        3,
        7,
        i64::MAX - 1,
        i64::MAX,
    ];

    for dividend in values {
        for divisor in values.into_iter().filter(|value| *value != 0) {
            let dividend_wide = i128::from(dividend);
            let divisor_wide = i128::from(divisor);
            let mut quotient = dividend_wide / divisor_wide;
            let mut remainder = dividend_wide % divisor_wide;
            if remainder != 0 && (remainder < 0) != (divisor_wide < 0) {
                quotient -= 1;
                remainder += divisor_wide;
            }

            assert_eq!(
                division(
                    MirIntegerDivisionKind::Quotient,
                    MirIntegerType::I64,
                    PrimitiveConstant::I64(dividend),
                    PrimitiveConstant::I64(divisor),
                ),
                success(PrimitiveConstant::I64(quotient as i64))
            );
            assert_eq!(
                division(
                    MirIntegerDivisionKind::Remainder,
                    MirIntegerType::I64,
                    PrimitiveConstant::I64(dividend),
                    PrimitiveConstant::I64(divisor),
                ),
                success(PrimitiveConstant::I64(remainder as i64))
            );
        }
    }
}

#[test]
fn unsigned_extrema_and_all_byte_inputs_use_exact_quotient_and_remainder() {
    for (dividend, divisor) in [(0, 1), (1, u64::MAX), (u64::MAX, 1), (u64::MAX, 3)] {
        assert_eq!(
            division(
                MirIntegerDivisionKind::Quotient,
                MirIntegerType::U64,
                PrimitiveConstant::U64(dividend),
                PrimitiveConstant::U64(divisor),
            ),
            success(PrimitiveConstant::U64(dividend / divisor))
        );
        assert_eq!(
            division(
                MirIntegerDivisionKind::Remainder,
                MirIntegerType::U64,
                PrimitiveConstant::U64(dividend),
                PrimitiveConstant::U64(divisor),
            ),
            success(PrimitiveConstant::U64(dividend % divisor))
        );
    }

    for dividend in u8::MIN..=u8::MAX {
        for divisor in 1..=u8::MAX {
            assert_eq!(
                division(
                    MirIntegerDivisionKind::Quotient,
                    MirIntegerType::U8,
                    PrimitiveConstant::U8(dividend),
                    PrimitiveConstant::U8(divisor),
                ),
                success(PrimitiveConstant::U8(dividend / divisor))
            );
            assert_eq!(
                division(
                    MirIntegerDivisionKind::Remainder,
                    MirIntegerType::U8,
                    PrimitiveConstant::U8(dividend),
                    PrimitiveConstant::U8(divisor),
                ),
                success(PrimitiveConstant::U8(dividend % divisor))
            );
        }
    }
}

#[test]
fn exact_zero_divisors_report_operation_specific_static_failures() {
    for (operand, dividend, divisor) in [
        (
            MirIntegerType::I64,
            PrimitiveConstant::I64(i64::MIN),
            PrimitiveConstant::I64(0),
        ),
        (
            MirIntegerType::U64,
            PrimitiveConstant::U64(u64::MAX),
            PrimitiveConstant::U64(0),
        ),
        (
            MirIntegerType::U8,
            PrimitiveConstant::U8(u8::MAX),
            PrimitiveConstant::U8(0),
        ),
    ] {
        assert_eq!(
            division(MirIntegerDivisionKind::Quotient, operand, dividend, divisor),
            CheckedIntegerEvaluation::Failure(MirTerminationReason::IntegerDivisionByZero)
        );
        assert_eq!(
            division(
                MirIntegerDivisionKind::Remainder,
                operand,
                dividend,
                divisor
            ),
            CheckedIntegerEvaluation::Failure(MirTerminationReason::IntegerRemainderByZero)
        );
    }
}

#[test]
fn division_type_mismatches_and_non_integer_constants_are_unsupported() {
    for kind in [
        MirIntegerDivisionKind::Quotient,
        MirIntegerDivisionKind::Remainder,
    ] {
        assert_eq!(
            division(
                kind,
                MirIntegerType::I64,
                PrimitiveConstant::U64(7),
                PrimitiveConstant::U64(3),
            ),
            CheckedIntegerEvaluation::Unsupported
        );
        assert_eq!(
            division(
                kind,
                MirIntegerType::U64,
                PrimitiveConstant::U64(7),
                PrimitiveConstant::Bool(false),
            ),
            CheckedIntegerEvaluation::Unsupported
        );
        assert_eq!(
            division(
                kind,
                MirIntegerType::U8,
                PrimitiveConstant::U8(7),
                PrimitiveConstant::U64(0),
            ),
            CheckedIntegerEvaluation::Unsupported
        );
    }
}

#[test]
fn shift_matrix_uses_exact_width_and_right_shift_flavor() {
    let cases = [
        (
            MirIntegerType::I64,
            PrimitiveConstant::I64(-2),
            PrimitiveConstant::I64(-4),
            PrimitiveConstant::I64(-1),
        ),
        (
            MirIntegerType::U64,
            PrimitiveConstant::U64(1_u64 << 63),
            PrimitiveConstant::U64(0),
            PrimitiveConstant::U64(1_u64 << 62),
        ),
        (
            MirIntegerType::U8,
            PrimitiveConstant::U8(0x81),
            PrimitiveConstant::U8(0x02),
            PrimitiveConstant::U8(0x40),
        ),
    ];

    for (left_type, left, shifted_left, shifted_right) in cases {
        assert_eq!(
            shift(
                MirShiftDirection::Left,
                left_type,
                left,
                PrimitiveConstant::U64(1),
            ),
            success(shifted_left)
        );
        assert_eq!(
            shift(
                MirShiftDirection::Right,
                left_type,
                left,
                PrimitiveConstant::U64(1),
            ),
            success(shifted_right)
        );
    }
}

#[test]
fn shifts_accept_zero_and_maximum_valid_counts_and_reject_larger_counts() {
    for (left_type, left, width) in [
        (MirIntegerType::I64, PrimitiveConstant::I64(-1), 64),
        (MirIntegerType::U64, PrimitiveConstant::U64(u64::MAX), 64),
        (MirIntegerType::U8, PrimitiveConstant::U8(u8::MAX), 8),
    ] {
        for direction in [MirShiftDirection::Left, MirShiftDirection::Right] {
            assert_eq!(
                shift(direction, left_type, left, PrimitiveConstant::U64(0)),
                success(left)
            );
            assert!(matches!(
                shift(
                    direction,
                    left_type,
                    left,
                    PrimitiveConstant::U64(width - 1),
                ),
                CheckedIntegerEvaluation::Success(_)
            ));
            for count in [width, width + 1, u64::MAX] {
                assert_eq!(
                    shift(direction, left_type, left, PrimitiveConstant::U64(count),),
                    CheckedIntegerEvaluation::Failure(MirTerminationReason::ShiftCountOutOfRange)
                );
            }
        }
    }
}

#[test]
fn every_byte_shift_result_is_canonical() {
    for left in u8::MIN..=u8::MAX {
        for count in 0..8 {
            assert_eq!(
                shift(
                    MirShiftDirection::Left,
                    MirIntegerType::U8,
                    PrimitiveConstant::U8(left),
                    PrimitiveConstant::U64(count),
                ),
                success(PrimitiveConstant::U8(left.wrapping_shl(count as u32)))
            );
            assert_eq!(
                shift(
                    MirShiftDirection::Right,
                    MirIntegerType::U8,
                    PrimitiveConstant::U8(left),
                    PrimitiveConstant::U64(count),
                ),
                success(PrimitiveConstant::U8(left.wrapping_shr(count as u32)))
            );
        }
    }
}

#[test]
fn shift_type_mismatches_and_non_u64_counts_are_unsupported() {
    for direction in [MirShiftDirection::Left, MirShiftDirection::Right] {
        assert_eq!(
            shift(
                direction,
                MirIntegerType::I64,
                PrimitiveConstant::U64(1),
                PrimitiveConstant::U64(1),
            ),
            CheckedIntegerEvaluation::Unsupported
        );
        assert_eq!(
            shift(
                direction,
                MirIntegerType::U64,
                PrimitiveConstant::U64(1),
                PrimitiveConstant::U8(1),
            ),
            CheckedIntegerEvaluation::Unsupported
        );
        assert_eq!(
            shift(
                direction,
                MirIntegerType::U8,
                PrimitiveConstant::Bool(true),
                PrimitiveConstant::U64(1),
            ),
            CheckedIntegerEvaluation::Unsupported
        );
    }
}
