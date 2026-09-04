//! Exact target-independent evaluation of checked integer operations.

use crate::mir::{
    MirIntegerDivisionKind, MirIntegerDivisionOperation, MirIntegerType, MirShiftDirection,
    MirShiftOperation, MirTerminationReason,
};

use super::primitive_evaluation::{canonical_u8, PrimitiveConstant};

/// Result of evaluating one checked integer operation from exact constants.
///
/// A static failure is distinct from an unsupported input: later protocol
/// rewriting may use only successful results, while retaining the exact reason
/// that prevented a statically failing operation from being folded.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CheckedIntegerEvaluation {
    Success(PrimitiveConstant),
    Failure(MirTerminationReason),
    Unsupported,
}

/// Evaluates an exact integer quotient or remainder operation.
///
/// Type mismatches are unsupported. An exact zero divisor reports the MIR
/// operation's failure reason without executing host division.
pub(super) fn evaluate_integer_division(
    operation: MirIntegerDivisionOperation,
    dividend: PrimitiveConstant,
    divisor: PrimitiveConstant,
) -> CheckedIntegerEvaluation {
    match (operation.operand, dividend, divisor) {
        (
            MirIntegerType::I64,
            PrimitiveConstant::I64(dividend),
            PrimitiveConstant::I64(divisor),
        ) => evaluate_i64_division(
            operation.kind,
            operation.failure_reason(),
            dividend,
            divisor,
        ),
        (
            MirIntegerType::U64,
            PrimitiveConstant::U64(dividend),
            PrimitiveConstant::U64(divisor),
        ) => evaluate_u64_division(
            operation.kind,
            operation.failure_reason(),
            dividend,
            divisor,
        ),
        (MirIntegerType::U8, PrimitiveConstant::U8(dividend), PrimitiveConstant::U8(divisor)) => {
            evaluate_u8_division(
                operation.kind,
                operation.failure_reason(),
                dividend,
                divisor,
            )
        }
        _ => CheckedIntegerEvaluation::Unsupported,
    }
}

/// Evaluates an exact integer shift after validating its `u64` count.
pub(super) fn evaluate_shift(
    operation: MirShiftOperation,
    left: PrimitiveConstant,
    count: PrimitiveConstant,
) -> CheckedIntegerEvaluation {
    let PrimitiveConstant::U64(count) = count else {
        return CheckedIntegerEvaluation::Unsupported;
    };
    if count >= operation.width() {
        return CheckedIntegerEvaluation::Failure(operation.failure_reason());
    }
    let count = count as u32;

    let result = match (operation.left, left) {
        (MirIntegerType::I64, PrimitiveConstant::I64(left)) => {
            PrimitiveConstant::I64(match operation.direction {
                MirShiftDirection::Left => left.wrapping_shl(count),
                MirShiftDirection::Right => left.wrapping_shr(count),
            })
        }
        (MirIntegerType::U64, PrimitiveConstant::U64(left)) => {
            PrimitiveConstant::U64(match operation.direction {
                MirShiftDirection::Left => left.wrapping_shl(count),
                MirShiftDirection::Right => left.wrapping_shr(count),
            })
        }
        (MirIntegerType::U8, PrimitiveConstant::U8(left)) => {
            let left = u64::from(left);
            let shifted = match operation.direction {
                MirShiftDirection::Left => left.wrapping_shl(count),
                MirShiftDirection::Right => left.wrapping_shr(count),
            };
            PrimitiveConstant::U8(canonical_u8(shifted))
        }
        _ => return CheckedIntegerEvaluation::Unsupported,
    };
    CheckedIntegerEvaluation::Success(result)
}

fn evaluate_i64_division(
    kind: MirIntegerDivisionKind,
    failure: MirTerminationReason,
    dividend: i64,
    divisor: i64,
) -> CheckedIntegerEvaluation {
    if divisor == 0 {
        return CheckedIntegerEvaluation::Failure(failure);
    }
    let (quotient, remainder) = floor_division_i64(dividend, divisor);
    CheckedIntegerEvaluation::Success(match kind {
        MirIntegerDivisionKind::Quotient => PrimitiveConstant::I64(quotient),
        MirIntegerDivisionKind::Remainder => PrimitiveConstant::I64(remainder),
    })
}

fn evaluate_u64_division(
    kind: MirIntegerDivisionKind,
    failure: MirTerminationReason,
    dividend: u64,
    divisor: u64,
) -> CheckedIntegerEvaluation {
    if divisor == 0 {
        return CheckedIntegerEvaluation::Failure(failure);
    }
    CheckedIntegerEvaluation::Success(match kind {
        MirIntegerDivisionKind::Quotient => PrimitiveConstant::U64(dividend / divisor),
        MirIntegerDivisionKind::Remainder => PrimitiveConstant::U64(dividend % divisor),
    })
}

fn evaluate_u8_division(
    kind: MirIntegerDivisionKind,
    failure: MirTerminationReason,
    dividend: u8,
    divisor: u8,
) -> CheckedIntegerEvaluation {
    if divisor == 0 {
        return CheckedIntegerEvaluation::Failure(failure);
    }
    let result = match kind {
        MirIntegerDivisionKind::Quotient => dividend / divisor,
        MirIntegerDivisionKind::Remainder => dividend % divisor,
    };
    CheckedIntegerEvaluation::Success(PrimitiveConstant::U8(canonical_u8(u64::from(result))))
}

/// Returns Skald's floor quotient and divisor-sign remainder without exposing
/// Rust's signed-minimum division overflow.
fn floor_division_i64(dividend: i64, divisor: i64) -> (i64, i64) {
    if dividend == i64::MIN && divisor == -1 {
        return (i64::MIN, 0);
    }

    let mut quotient = dividend / divisor;
    let mut remainder = dividend % divisor;
    if remainder != 0 && (remainder < 0) != (divisor < 0) {
        quotient = quotient.wrapping_sub(1);
        remainder = remainder.wrapping_add(divisor);
    }
    (quotient, remainder)
}

#[cfg(test)]
#[path = "checked_integer_evaluation/tests.rs"]
mod tests;
