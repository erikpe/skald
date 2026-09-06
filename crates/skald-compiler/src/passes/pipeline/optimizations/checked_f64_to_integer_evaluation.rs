//! Exact target-independent evaluation of checked floating-to-integer casts.

use skald_binary64::{Binary64, IntegerConversion};

use crate::mir::{MirF64ToIntegerRange, MirIntegerType, MirTerminationReason};

use super::primitive_evaluation::PrimitiveConstant;

/// Result of evaluating one checked floating cast from an exact source fact.
///
/// Static failure remains distinct from an unsupported or mismatched input so
/// the solver can retain the executable failure protocol without publishing a
/// result fact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::passes) enum CheckedF64ToIntegerEvaluation {
    Success(PrimitiveConstant),
    Failure(MirTerminationReason),
    Unsupported,
}

pub(in crate::passes) fn evaluate_f64_to_integer(
    relation: MirF64ToIntegerRange,
    source: PrimitiveConstant,
) -> CheckedF64ToIntegerEvaluation {
    let PrimitiveConstant::F64Bits(bits) = source else {
        return CheckedF64ToIntegerEvaluation::Unsupported;
    };
    let value = Binary64::from_bits(bits);
    match relation.target {
        MirIntegerType::I64 => conversion_result(
            value.truncating_to_i64(),
            PrimitiveConstant::I64,
            relation.failure_reason(),
        ),
        MirIntegerType::U64 => conversion_result(
            value.truncating_to_u64(),
            PrimitiveConstant::U64,
            relation.failure_reason(),
        ),
        MirIntegerType::U8 => conversion_result(
            value.truncating_to_u8(),
            PrimitiveConstant::U8,
            relation.failure_reason(),
        ),
    }
}

fn conversion_result<T>(
    result: IntegerConversion<T>,
    constant: impl FnOnce(T) -> PrimitiveConstant,
    failure: MirTerminationReason,
) -> CheckedF64ToIntegerEvaluation {
    match result {
        IntegerConversion::Value(value) => CheckedF64ToIntegerEvaluation::Success(constant(value)),
        IntegerConversion::OutOfRange => CheckedF64ToIntegerEvaluation::Failure(failure),
    }
}

#[cfg(test)]
#[path = "checked_f64_to_integer_evaluation/tests.rs"]
mod tests;
