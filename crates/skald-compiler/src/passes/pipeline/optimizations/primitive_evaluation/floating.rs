//! Exact binary64 evaluation behind the compiler's primitive fact boundary.

use skald_binary64::{Binary64, Binary64Comparison};

use crate::mir::{
    MirBinaryOperation, MirComparisonPredicate, MirPrimitiveCast, MirPrimitiveCastKind,
    MirPrimitiveType,
};

use super::{constant, PrimitiveConstant, PrimitiveEvaluation};

pub(super) fn evaluate_negation(operand: PrimitiveConstant) -> PrimitiveEvaluation {
    let PrimitiveConstant::F64Bits(bits) = operand else {
        return PrimitiveEvaluation::Unsupported;
    };
    binary64(Binary64::from_bits(bits).negate())
}

pub(super) fn evaluate_binary(
    operation: MirBinaryOperation,
    left: PrimitiveConstant,
    right: PrimitiveConstant,
) -> PrimitiveEvaluation {
    let (PrimitiveConstant::F64Bits(left), PrimitiveConstant::F64Bits(right)) = (left, right)
    else {
        return PrimitiveEvaluation::Unsupported;
    };
    let left = Binary64::from_bits(left);
    let right = Binary64::from_bits(right);
    let result = match operation {
        MirBinaryOperation::AddF64 => left.add(right),
        MirBinaryOperation::SubtractF64 => left.subtract(right),
        MirBinaryOperation::MultiplyF64 => left.multiply(right),
        MirBinaryOperation::DivideF64 => left.divide(right),
        MirBinaryOperation::AddI64
        | MirBinaryOperation::SubtractI64
        | MirBinaryOperation::MultiplyI64
        | MirBinaryOperation::AddU64
        | MirBinaryOperation::SubtractU64
        | MirBinaryOperation::MultiplyU64
        | MirBinaryOperation::AddU8
        | MirBinaryOperation::SubtractU8
        | MirBinaryOperation::MultiplyU8
        | MirBinaryOperation::IntegerBitwise { .. } => {
            return PrimitiveEvaluation::Unsupported;
        }
    };
    if result.is_nan() {
        PrimitiveEvaluation::Unsupported
    } else {
        binary64(result)
    }
}

pub(super) fn evaluate_comparison(
    predicate: MirComparisonPredicate,
    left: PrimitiveConstant,
    right: PrimitiveConstant,
) -> PrimitiveEvaluation {
    let (PrimitiveConstant::F64Bits(left), PrimitiveConstant::F64Bits(right)) = (left, right)
    else {
        return PrimitiveEvaluation::Unsupported;
    };
    let ordering = Binary64::from_bits(left).compare(Binary64::from_bits(right));
    let result = match (predicate, ordering) {
        (MirComparisonPredicate::Equal, Binary64Comparison::Equal)
        | (MirComparisonPredicate::NotEqual, Binary64Comparison::Less)
        | (MirComparisonPredicate::NotEqual, Binary64Comparison::Greater)
        | (MirComparisonPredicate::NotEqual, Binary64Comparison::Unordered)
        | (MirComparisonPredicate::LessThan, Binary64Comparison::Less)
        | (
            MirComparisonPredicate::LessEqual,
            Binary64Comparison::Less | Binary64Comparison::Equal,
        )
        | (MirComparisonPredicate::GreaterThan, Binary64Comparison::Greater)
        | (
            MirComparisonPredicate::GreaterEqual,
            Binary64Comparison::Equal | Binary64Comparison::Greater,
        ) => true,
        (MirComparisonPredicate::Equal, _)
        | (MirComparisonPredicate::NotEqual, Binary64Comparison::Equal)
        | (MirComparisonPredicate::LessThan, _)
        | (MirComparisonPredicate::LessEqual, _)
        | (MirComparisonPredicate::GreaterThan, _)
        | (MirComparisonPredicate::GreaterEqual, _) => false,
    };
    constant(PrimitiveConstant::Bool(result))
}

pub(super) fn evaluate_cast(
    operation: MirPrimitiveCast,
    operand: PrimitiveConstant,
) -> PrimitiveEvaluation {
    match (
        operation.kind(),
        operation.source,
        operation.target,
        operand,
    ) {
        (
            MirPrimitiveCastKind::ToBool,
            MirPrimitiveType::F64,
            MirPrimitiveType::Bool,
            PrimitiveConstant::F64Bits(bits),
        ) => constant(PrimitiveConstant::Bool(Binary64::from_bits(bits).to_bool())),
        (
            MirPrimitiveCastKind::ToF64,
            MirPrimitiveType::I64,
            MirPrimitiveType::F64,
            PrimitiveConstant::I64(value),
        ) => binary64(Binary64::from_i64(value)),
        (
            MirPrimitiveCastKind::ToF64,
            MirPrimitiveType::U64,
            MirPrimitiveType::F64,
            PrimitiveConstant::U64(value),
        ) => binary64(Binary64::from_u64(value)),
        (
            MirPrimitiveCastKind::ToF64,
            MirPrimitiveType::U8,
            MirPrimitiveType::F64,
            PrimitiveConstant::U8(value),
        ) => binary64(Binary64::from_u8(value)),
        (
            MirPrimitiveCastKind::ToF64,
            MirPrimitiveType::Bool,
            MirPrimitiveType::F64,
            PrimitiveConstant::Bool(value),
        ) => binary64(Binary64::from_bool(value)),
        (
            MirPrimitiveCastKind::BitReinterpretation,
            MirPrimitiveType::U64,
            MirPrimitiveType::F64,
            PrimitiveConstant::U64(bits),
        ) => constant(PrimitiveConstant::F64Bits(bits)),
        (
            MirPrimitiveCastKind::BitReinterpretation,
            MirPrimitiveType::F64,
            MirPrimitiveType::U64,
            PrimitiveConstant::F64Bits(bits),
        ) => constant(PrimitiveConstant::U64(bits)),
        _ => PrimitiveEvaluation::Unsupported,
    }
}

fn binary64(value: Binary64) -> PrimitiveEvaluation {
    constant(PrimitiveConstant::F64Bits(value.to_bits()))
}
