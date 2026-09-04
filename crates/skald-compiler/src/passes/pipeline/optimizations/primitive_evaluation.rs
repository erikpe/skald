//! Exact target-independent evaluation of the foldable primitive MIR subset.

use crate::mir::{
    MirBinaryOperation, MirComparisonOperand, MirComparisonPredicate, MirIntegerBitwiseOperation,
    MirIntegerType, MirPrimitiveCast, MirPrimitiveCastKind, MirPrimitiveComparison,
    MirPrimitiveType, MirRvalueKind, MirType, MirUnaryOperation, ValueId,
};

/// One exact constant in the deliberately closed local-simplification domain.
///
/// This is an optimizer fact, not an alternative MIR type system. MIR remains
/// the owner of value types and operations; this representation merely keeps
/// the payload paired with its exact supported type while facts are built.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PrimitiveConstant {
    I64(i64),
    U64(u64),
    U8(u8),
    Bool(bool),
}

impl PrimitiveConstant {
    pub(super) const fn ty(self) -> MirType {
        self.primitive_type().value_type()
    }

    const fn primitive_type(self) -> MirPrimitiveType {
        match self {
            Self::I64(_) => MirPrimitiveType::I64,
            Self::U64(_) => MirPrimitiveType::U64,
            Self::U8(_) => MirPrimitiveType::U8,
            Self::Bool(_) => MirPrimitiveType::Bool,
        }
    }

    pub(super) const fn into_rvalue_kind(self) -> MirRvalueKind {
        match self {
            Self::I64(value) => MirRvalueKind::ConstantI64(value),
            Self::U64(value) => MirRvalueKind::ConstantU64(value),
            Self::U8(value) => MirRvalueKind::ConstantU8(value),
            Self::Bool(value) => MirRvalueKind::ConstantBool(value),
        }
    }
}

/// Result of evaluating one MIR rvalue in the closed primitive domain.
///
/// Unsupported is an ordinary conservative outcome. It covers type
/// mismatches as well as every operation family outside the frozen set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PrimitiveEvaluation {
    Constant(PrimitiveConstant),
    Unsupported,
}

/// Evaluates one rvalue using constants already known for its value operands.
///
/// The operation matches are intentionally exhaustive. Adding a MIR rvalue or
/// primitive operation therefore requires an explicit folding decision here.
pub(super) fn evaluate_rvalue(
    kind: &MirRvalueKind,
    mut known_constant: impl FnMut(ValueId) -> Option<PrimitiveConstant>,
) -> PrimitiveEvaluation {
    match kind {
        MirRvalueKind::ConstantI64(value) => constant(PrimitiveConstant::I64(*value)),
        MirRvalueKind::ConstantU64(value) => constant(PrimitiveConstant::U64(*value)),
        MirRvalueKind::ConstantU8(value) => constant(PrimitiveConstant::U8(*value)),
        MirRvalueKind::ConstantBool(value) => constant(PrimitiveConstant::Bool(*value)),
        MirRvalueKind::ConstantF64Bits(_) => PrimitiveEvaluation::Unsupported,
        MirRvalueKind::Unary { operation, operand } => known_constant(*operand)
            .map_or(PrimitiveEvaluation::Unsupported, |operand| {
                evaluate_unary(*operation, operand)
            }),
        MirRvalueKind::Binary {
            operation,
            left,
            right,
        } => match (known_constant(*left), known_constant(*right)) {
            (Some(left), Some(right)) => evaluate_binary(*operation, left, right),
            _ => PrimitiveEvaluation::Unsupported,
        },
        MirRvalueKind::PrimitiveComparison {
            operation,
            left,
            right,
        } => match (known_constant(*left), known_constant(*right)) {
            (Some(left), Some(right)) => evaluate_comparison(*operation, left, right),
            _ => PrimitiveEvaluation::Unsupported,
        },
        MirRvalueKind::PrimitiveCast { operation, operand } => known_constant(*operand)
            .map_or(PrimitiveEvaluation::Unsupported, |operand| {
                evaluate_cast(*operation, operand)
            }),
        MirRvalueKind::CallableAddress(_)
        | MirRvalueKind::PathCondition(_)
        | MirRvalueKind::Load(_)
        | MirRvalueKind::IntegerDivision { .. }
        | MirRvalueKind::Shift { .. }
        | MirRvalueKind::CheckedF64ToInteger { .. }
        | MirRvalueKind::TypeTest { .. }
        | MirRvalueKind::OptionalPresence { .. }
        | MirRvalueKind::OptionalBoxPresence { .. }
        | MirRvalueKind::ArrayLength { .. } => PrimitiveEvaluation::Unsupported,
    }
}

const fn constant(value: PrimitiveConstant) -> PrimitiveEvaluation {
    PrimitiveEvaluation::Constant(value)
}

fn evaluate_unary(operation: MirUnaryOperation, operand: PrimitiveConstant) -> PrimitiveEvaluation {
    match operation {
        MirUnaryOperation::NegateI64 => match operand {
            PrimitiveConstant::I64(value) => constant(PrimitiveConstant::I64(value.wrapping_neg())),
            PrimitiveConstant::U64(_) | PrimitiveConstant::U8(_) | PrimitiveConstant::Bool(_) => {
                PrimitiveEvaluation::Unsupported
            }
        },
        MirUnaryOperation::NegateF64 => PrimitiveEvaluation::Unsupported,
        MirUnaryOperation::LogicalNotBool => match operand {
            PrimitiveConstant::Bool(value) => constant(PrimitiveConstant::Bool(!value)),
            PrimitiveConstant::I64(_) | PrimitiveConstant::U64(_) | PrimitiveConstant::U8(_) => {
                PrimitiveEvaluation::Unsupported
            }
        },
        MirUnaryOperation::BitwiseComplement(integer) => complement(integer, operand),
    }
}

fn complement(integer: MirIntegerType, operand: PrimitiveConstant) -> PrimitiveEvaluation {
    match integer {
        MirIntegerType::I64 => match operand {
            PrimitiveConstant::I64(value) => constant(PrimitiveConstant::I64(!value)),
            PrimitiveConstant::U64(_) | PrimitiveConstant::U8(_) | PrimitiveConstant::Bool(_) => {
                PrimitiveEvaluation::Unsupported
            }
        },
        MirIntegerType::U64 => match operand {
            PrimitiveConstant::U64(value) => constant(PrimitiveConstant::U64(!value)),
            PrimitiveConstant::I64(_) | PrimitiveConstant::U8(_) | PrimitiveConstant::Bool(_) => {
                PrimitiveEvaluation::Unsupported
            }
        },
        MirIntegerType::U8 => match operand {
            PrimitiveConstant::U8(value) => {
                constant(PrimitiveConstant::U8(canonical_u8(!u64::from(value))))
            }
            PrimitiveConstant::I64(_) | PrimitiveConstant::U64(_) | PrimitiveConstant::Bool(_) => {
                PrimitiveEvaluation::Unsupported
            }
        },
    }
}

fn evaluate_binary(
    operation: MirBinaryOperation,
    left: PrimitiveConstant,
    right: PrimitiveConstant,
) -> PrimitiveEvaluation {
    match operation {
        MirBinaryOperation::AddI64 => binary_i64(left, right, i64::wrapping_add),
        MirBinaryOperation::SubtractI64 => binary_i64(left, right, i64::wrapping_sub),
        MirBinaryOperation::MultiplyI64 => binary_i64(left, right, i64::wrapping_mul),
        MirBinaryOperation::AddU64 => binary_u64(left, right, u64::wrapping_add),
        MirBinaryOperation::SubtractU64 => binary_u64(left, right, u64::wrapping_sub),
        MirBinaryOperation::MultiplyU64 => binary_u64(left, right, u64::wrapping_mul),
        MirBinaryOperation::AddU8 => binary_u8(left, right, u64::wrapping_add),
        MirBinaryOperation::SubtractU8 => binary_u8(left, right, u64::wrapping_sub),
        MirBinaryOperation::MultiplyU8 => binary_u8(left, right, u64::wrapping_mul),
        MirBinaryOperation::AddF64
        | MirBinaryOperation::SubtractF64
        | MirBinaryOperation::MultiplyF64
        | MirBinaryOperation::DivideF64 => PrimitiveEvaluation::Unsupported,
        MirBinaryOperation::IntegerBitwise { operation, operand } => {
            evaluate_bitwise(operation, operand, left, right)
        }
    }
}

fn binary_i64(
    left: PrimitiveConstant,
    right: PrimitiveConstant,
    operation: fn(i64, i64) -> i64,
) -> PrimitiveEvaluation {
    match (left, right) {
        (PrimitiveConstant::I64(left), PrimitiveConstant::I64(right)) => {
            constant(PrimitiveConstant::I64(operation(left, right)))
        }
        _ => PrimitiveEvaluation::Unsupported,
    }
}

fn binary_u64(
    left: PrimitiveConstant,
    right: PrimitiveConstant,
    operation: fn(u64, u64) -> u64,
) -> PrimitiveEvaluation {
    match (left, right) {
        (PrimitiveConstant::U64(left), PrimitiveConstant::U64(right)) => {
            constant(PrimitiveConstant::U64(operation(left, right)))
        }
        _ => PrimitiveEvaluation::Unsupported,
    }
}

fn binary_u8(
    left: PrimitiveConstant,
    right: PrimitiveConstant,
    operation: fn(u64, u64) -> u64,
) -> PrimitiveEvaluation {
    match (left, right) {
        (PrimitiveConstant::U8(left), PrimitiveConstant::U8(right)) => {
            let result = operation(u64::from(left), u64::from(right));
            constant(PrimitiveConstant::U8(canonical_u8(result)))
        }
        _ => PrimitiveEvaluation::Unsupported,
    }
}

fn evaluate_bitwise(
    operation: MirIntegerBitwiseOperation,
    operand: MirIntegerType,
    left: PrimitiveConstant,
    right: PrimitiveConstant,
) -> PrimitiveEvaluation {
    match operand {
        MirIntegerType::I64 => match (left, right) {
            (PrimitiveConstant::I64(left), PrimitiveConstant::I64(right)) => {
                constant(PrimitiveConstant::I64(bitwise_i64(operation, left, right)))
            }
            _ => PrimitiveEvaluation::Unsupported,
        },
        MirIntegerType::U64 => match (left, right) {
            (PrimitiveConstant::U64(left), PrimitiveConstant::U64(right)) => {
                constant(PrimitiveConstant::U64(bitwise_u64(operation, left, right)))
            }
            _ => PrimitiveEvaluation::Unsupported,
        },
        MirIntegerType::U8 => match (left, right) {
            (PrimitiveConstant::U8(left), PrimitiveConstant::U8(right)) => {
                let result = bitwise_u64(operation, u64::from(left), u64::from(right));
                constant(PrimitiveConstant::U8(canonical_u8(result)))
            }
            _ => PrimitiveEvaluation::Unsupported,
        },
    }
}

const fn bitwise_i64(operation: MirIntegerBitwiseOperation, left: i64, right: i64) -> i64 {
    match operation {
        MirIntegerBitwiseOperation::And => left & right,
        MirIntegerBitwiseOperation::Or => left | right,
        MirIntegerBitwiseOperation::Xor => left ^ right,
    }
}

const fn bitwise_u64(operation: MirIntegerBitwiseOperation, left: u64, right: u64) -> u64 {
    match operation {
        MirIntegerBitwiseOperation::And => left & right,
        MirIntegerBitwiseOperation::Or => left | right,
        MirIntegerBitwiseOperation::Xor => left ^ right,
    }
}

fn evaluate_comparison(
    operation: MirPrimitiveComparison,
    left: PrimitiveConstant,
    right: PrimitiveConstant,
) -> PrimitiveEvaluation {
    match operation.operand {
        MirComparisonOperand::Integer(MirIntegerType::I64) => match (left, right) {
            (PrimitiveConstant::I64(left), PrimitiveConstant::I64(right)) => {
                ordered_comparison(operation.predicate, left, right)
            }
            _ => PrimitiveEvaluation::Unsupported,
        },
        MirComparisonOperand::Integer(MirIntegerType::U64) => match (left, right) {
            (PrimitiveConstant::U64(left), PrimitiveConstant::U64(right)) => {
                ordered_comparison(operation.predicate, left, right)
            }
            _ => PrimitiveEvaluation::Unsupported,
        },
        MirComparisonOperand::Integer(MirIntegerType::U8) => match (left, right) {
            (PrimitiveConstant::U8(left), PrimitiveConstant::U8(right)) => {
                ordered_comparison(operation.predicate, left, right)
            }
            _ => PrimitiveEvaluation::Unsupported,
        },
        MirComparisonOperand::Bool => match (operation.predicate, left, right) {
            (
                MirComparisonPredicate::Equal,
                PrimitiveConstant::Bool(left),
                PrimitiveConstant::Bool(right),
            ) => constant(PrimitiveConstant::Bool(left == right)),
            (
                MirComparisonPredicate::NotEqual,
                PrimitiveConstant::Bool(left),
                PrimitiveConstant::Bool(right),
            ) => constant(PrimitiveConstant::Bool(left != right)),
            (MirComparisonPredicate::LessThan, _, _)
            | (MirComparisonPredicate::LessEqual, _, _)
            | (MirComparisonPredicate::GreaterThan, _, _)
            | (MirComparisonPredicate::GreaterEqual, _, _)
            | (MirComparisonPredicate::Equal | MirComparisonPredicate::NotEqual, _, _) => {
                PrimitiveEvaluation::Unsupported
            }
        },
        MirComparisonOperand::F64 => PrimitiveEvaluation::Unsupported,
    }
}

fn ordered_comparison<T: Ord>(
    predicate: MirComparisonPredicate,
    left: T,
    right: T,
) -> PrimitiveEvaluation {
    let result = match predicate {
        MirComparisonPredicate::Equal => left == right,
        MirComparisonPredicate::NotEqual => left != right,
        MirComparisonPredicate::LessThan => left < right,
        MirComparisonPredicate::LessEqual => left <= right,
        MirComparisonPredicate::GreaterThan => left > right,
        MirComparisonPredicate::GreaterEqual => left >= right,
    };
    constant(PrimitiveConstant::Bool(result))
}

fn evaluate_cast(operation: MirPrimitiveCast, operand: PrimitiveConstant) -> PrimitiveEvaluation {
    match operation.kind() {
        MirPrimitiveCastKind::Identity => {
            if operation.source == operation.target && operand.primitive_type() == operation.source
            {
                constant(operand)
            } else {
                PrimitiveEvaluation::Unsupported
            }
        }
        MirPrimitiveCastKind::IntegerBits => {
            evaluate_integer_bits(operation.source, operation.target, operand)
        }
        MirPrimitiveCastKind::ToBool => {
            evaluate_integer_to_bool(operation.source, operation.target, operand)
        }
        MirPrimitiveCastKind::FromBool => {
            evaluate_bool_to_integer(operation.source, operation.target, operand)
        }
        MirPrimitiveCastKind::ToF64
        | MirPrimitiveCastKind::BitReinterpretation
        | MirPrimitiveCastKind::CheckedF64ToInteger => PrimitiveEvaluation::Unsupported,
    }
}

fn evaluate_integer_bits(
    source: MirPrimitiveType,
    target: MirPrimitiveType,
    operand: PrimitiveConstant,
) -> PrimitiveEvaluation {
    let bits = match (source, operand) {
        (MirPrimitiveType::I64, PrimitiveConstant::I64(value)) => value as u64,
        (MirPrimitiveType::U64, PrimitiveConstant::U64(value)) => value,
        (MirPrimitiveType::U8, PrimitiveConstant::U8(value)) => u64::from(value),
        (MirPrimitiveType::F64 | MirPrimitiveType::Bool, _)
        | (MirPrimitiveType::I64, _)
        | (MirPrimitiveType::U64, _)
        | (MirPrimitiveType::U8, _) => return PrimitiveEvaluation::Unsupported,
    };

    match target {
        MirPrimitiveType::I64 => constant(PrimitiveConstant::I64(bits as i64)),
        MirPrimitiveType::U64 => constant(PrimitiveConstant::U64(bits)),
        MirPrimitiveType::U8 => constant(PrimitiveConstant::U8(canonical_u8(bits))),
        MirPrimitiveType::F64 | MirPrimitiveType::Bool => PrimitiveEvaluation::Unsupported,
    }
}

fn evaluate_integer_to_bool(
    source: MirPrimitiveType,
    target: MirPrimitiveType,
    operand: PrimitiveConstant,
) -> PrimitiveEvaluation {
    if target != MirPrimitiveType::Bool {
        return PrimitiveEvaluation::Unsupported;
    }
    let value = match (source, operand) {
        (MirPrimitiveType::I64, PrimitiveConstant::I64(value)) => value != 0,
        (MirPrimitiveType::U64, PrimitiveConstant::U64(value)) => value != 0,
        (MirPrimitiveType::U8, PrimitiveConstant::U8(value)) => value != 0,
        (MirPrimitiveType::F64 | MirPrimitiveType::Bool, _)
        | (MirPrimitiveType::I64, _)
        | (MirPrimitiveType::U64, _)
        | (MirPrimitiveType::U8, _) => return PrimitiveEvaluation::Unsupported,
    };
    constant(PrimitiveConstant::Bool(value))
}

fn evaluate_bool_to_integer(
    source: MirPrimitiveType,
    target: MirPrimitiveType,
    operand: PrimitiveConstant,
) -> PrimitiveEvaluation {
    let value = match (source, operand) {
        (MirPrimitiveType::Bool, PrimitiveConstant::Bool(value)) => u64::from(value),
        (MirPrimitiveType::I64, _)
        | (MirPrimitiveType::U64, _)
        | (MirPrimitiveType::U8, _)
        | (MirPrimitiveType::F64, _)
        | (MirPrimitiveType::Bool, _) => return PrimitiveEvaluation::Unsupported,
    };
    match target {
        MirPrimitiveType::I64 => constant(PrimitiveConstant::I64(value as i64)),
        MirPrimitiveType::U64 => constant(PrimitiveConstant::U64(value)),
        MirPrimitiveType::U8 => constant(PrimitiveConstant::U8(canonical_u8(value))),
        MirPrimitiveType::F64 | MirPrimitiveType::Bool => PrimitiveEvaluation::Unsupported,
    }
}

pub(super) const fn canonical_u8(bits: u64) -> u8 {
    (bits & u8::MAX as u64) as u8
}

#[cfg(test)]
#[path = "primitive_evaluation/tests.rs"]
mod tests;
