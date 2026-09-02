//! Frozen primitive integer and boolean algebraic identity catalog.

use std::collections::BTreeMap;

use crate::mir::{
    MirBinaryOperation, MirComparisonOperand, MirComparisonPredicate, MirIntegerBitwiseOperation,
    MirIntegerType, MirPrimitiveComparison, MirRvalueKind, MirType, MirUnaryOperation, ValueId,
};

use super::{primitive_evaluation::PrimitiveConstant, primitive_facts::PrimitiveConstantFacts};

/// Exact replacement selected by the reviewed identity catalog.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PrimitiveAlgebraicReplacement {
    Constant(PrimitiveConstant),
    Forward(ValueId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PrimitiveUnaryDefinition {
    operation: MirUnaryOperation,
    operand: ValueId,
}

/// Instruction-ordered facts needed by the algebraic catalog in one block.
#[derive(Debug, Default)]
pub(super) struct PrimitiveAlgebraicFacts {
    constants: PrimitiveConstantFacts,
    unary_definitions: BTreeMap<ValueId, PrimitiveUnaryDefinition>,
}

impl PrimitiveAlgebraicFacts {
    pub(super) fn begin_block(&mut self) {
        self.constants.begin_block();
        self.unary_definitions.clear();
    }

    pub(super) fn replacement(
        &self,
        kind: &MirRvalueKind,
        ty: MirType,
    ) -> Option<PrimitiveAlgebraicReplacement> {
        catalog_replacement(
            kind,
            ty,
            |value| self.constants.constant(value),
            |value| self.unary_definitions.get(&value).copied(),
        )
    }

    pub(super) fn observe_assignment(&mut self, assignment: &crate::mir::MirAssignment) {
        self.constants.observe_assignment(assignment);
        if let MirRvalueKind::Unary { operation, operand } = &assignment.rvalue.kind {
            self.unary_definitions.insert(
                assignment.result,
                PrimitiveUnaryDefinition {
                    operation: *operation,
                    operand: *operand,
                },
            );
        }
    }
}

fn catalog_replacement(
    kind: &MirRvalueKind,
    ty: MirType,
    mut constant: impl FnMut(ValueId) -> Option<PrimitiveConstant>,
    mut unary_definition: impl FnMut(ValueId) -> Option<PrimitiveUnaryDefinition>,
) -> Option<PrimitiveAlgebraicReplacement> {
    match kind {
        MirRvalueKind::Binary {
            operation,
            left,
            right,
        } => binary_replacement(*operation, *left, *right, ty, &mut constant),
        MirRvalueKind::PrimitiveComparison {
            operation,
            left,
            right,
        } => comparison_replacement(*operation, *left, *right),
        MirRvalueKind::Unary { operation, operand } => {
            unary_replacement(*operation, unary_definition(*operand))
        }
        MirRvalueKind::ConstantI64(_)
        | MirRvalueKind::ConstantU64(_)
        | MirRvalueKind::ConstantU8(_)
        | MirRvalueKind::ConstantF64Bits(_)
        | MirRvalueKind::ConstantBool(_)
        | MirRvalueKind::CallableAddress(_)
        | MirRvalueKind::PathCondition(_)
        | MirRvalueKind::Load(_)
        | MirRvalueKind::IntegerDivision { .. }
        | MirRvalueKind::Shift { .. }
        | MirRvalueKind::PrimitiveCast { .. }
        | MirRvalueKind::CheckedF64ToInteger { .. }
        | MirRvalueKind::TypeTest { .. }
        | MirRvalueKind::OptionalPresence { .. }
        | MirRvalueKind::OptionalBoxPresence { .. }
        | MirRvalueKind::ArrayLength { .. } => None,
    }
}

fn binary_replacement(
    operation: MirBinaryOperation,
    left: ValueId,
    right: ValueId,
    ty: MirType,
    constant: &mut impl FnMut(ValueId) -> Option<PrimitiveConstant>,
) -> Option<PrimitiveAlgebraicReplacement> {
    let left_constant = constant(left);
    let right_constant = constant(right);
    let forward = PrimitiveAlgebraicReplacement::Forward;
    let constant = PrimitiveAlgebraicReplacement::Constant;

    match operation {
        MirBinaryOperation::AddI64 | MirBinaryOperation::AddU64 | MirBinaryOperation::AddU8 => {
            if is_zero(right_constant, ty) {
                Some(forward(left))
            } else if is_zero(left_constant, ty) {
                Some(forward(right))
            } else {
                None
            }
        }
        MirBinaryOperation::SubtractI64
        | MirBinaryOperation::SubtractU64
        | MirBinaryOperation::SubtractU8 => {
            if left == right {
                zero(ty).map(constant)
            } else if is_zero(right_constant, ty) {
                Some(forward(left))
            } else {
                None
            }
        }
        MirBinaryOperation::MultiplyI64
        | MirBinaryOperation::MultiplyU64
        | MirBinaryOperation::MultiplyU8 => {
            if is_zero(left_constant, ty) || is_zero(right_constant, ty) {
                zero(ty).map(constant)
            } else if is_one(right_constant, ty) {
                Some(forward(left))
            } else if is_one(left_constant, ty) {
                Some(forward(right))
            } else {
                None
            }
        }
        MirBinaryOperation::IntegerBitwise { operation, operand } => bitwise_replacement(
            operation,
            operand,
            left,
            right,
            left_constant,
            right_constant,
        ),
        MirBinaryOperation::AddF64
        | MirBinaryOperation::SubtractF64
        | MirBinaryOperation::MultiplyF64
        | MirBinaryOperation::DivideF64 => None,
    }
}

fn bitwise_replacement(
    operation: MirIntegerBitwiseOperation,
    integer: MirIntegerType,
    left: ValueId,
    right: ValueId,
    left_constant: Option<PrimitiveConstant>,
    right_constant: Option<PrimitiveConstant>,
) -> Option<PrimitiveAlgebraicReplacement> {
    let ty = integer.operand_type();
    let forward = PrimitiveAlgebraicReplacement::Forward;
    let constant = PrimitiveAlgebraicReplacement::Constant;

    match operation {
        MirIntegerBitwiseOperation::And => {
            if is_zero(left_constant, ty) || is_zero(right_constant, ty) {
                zero(ty).map(constant)
            } else if left == right || is_all_ones(right_constant, ty) {
                Some(forward(left))
            } else if is_all_ones(left_constant, ty) {
                Some(forward(right))
            } else {
                None
            }
        }
        MirIntegerBitwiseOperation::Or => {
            if is_all_ones(left_constant, ty) || is_all_ones(right_constant, ty) {
                all_ones(ty).map(constant)
            } else if left == right || is_zero(right_constant, ty) {
                Some(forward(left))
            } else if is_zero(left_constant, ty) {
                Some(forward(right))
            } else {
                None
            }
        }
        MirIntegerBitwiseOperation::Xor => {
            if left == right {
                zero(ty).map(constant)
            } else if is_zero(right_constant, ty) {
                Some(forward(left))
            } else if is_zero(left_constant, ty) {
                Some(forward(right))
            } else {
                None
            }
        }
    }
}

fn comparison_replacement(
    operation: MirPrimitiveComparison,
    left: ValueId,
    right: ValueId,
) -> Option<PrimitiveAlgebraicReplacement> {
    let supported_operand = match operation.operand {
        MirComparisonOperand::Integer(_) | MirComparisonOperand::Bool => true,
        MirComparisonOperand::F64 => false,
    };
    if left != right
        || !supported_operand
        || !matches!(
            operation.predicate,
            MirComparisonPredicate::Equal | MirComparisonPredicate::NotEqual
        )
    {
        return None;
    }

    let result = matches!(operation.predicate, MirComparisonPredicate::Equal);
    Some(PrimitiveAlgebraicReplacement::Constant(
        PrimitiveConstant::Bool(result),
    ))
}

fn unary_replacement(
    operation: MirUnaryOperation,
    definition: Option<PrimitiveUnaryDefinition>,
) -> Option<PrimitiveAlgebraicReplacement> {
    let PrimitiveUnaryDefinition {
        operation: inner,
        operand: source,
    } = definition?;

    let is_involution = match (operation, inner) {
        (MirUnaryOperation::NegateI64, MirUnaryOperation::NegateI64)
        | (MirUnaryOperation::LogicalNotBool, MirUnaryOperation::LogicalNotBool) => true,
        (
            MirUnaryOperation::BitwiseComplement(outer),
            MirUnaryOperation::BitwiseComplement(inner),
        ) => outer == inner,
        (MirUnaryOperation::NegateF64, _)
        | (_, MirUnaryOperation::NegateF64)
        | (MirUnaryOperation::NegateI64, _)
        | (MirUnaryOperation::LogicalNotBool, _)
        | (MirUnaryOperation::BitwiseComplement(_), _) => false,
    };
    is_involution.then_some(PrimitiveAlgebraicReplacement::Forward(source))
}

const fn zero(ty: MirType) -> Option<PrimitiveConstant> {
    match ty {
        MirType::I64 => Some(PrimitiveConstant::I64(0)),
        MirType::U64 => Some(PrimitiveConstant::U64(0)),
        MirType::U8 => Some(PrimitiveConstant::U8(0)),
        MirType::F64
        | MirType::Bool
        | MirType::Function(_)
        | MirType::Array(_)
        | MirType::Class(_)
        | MirType::Interface(_)
        | MirType::Obj
        | MirType::Shared(_)
        | MirType::Optional(_)
        | MirType::Unit => None,
    }
}

const fn one(ty: MirType) -> Option<PrimitiveConstant> {
    match ty {
        MirType::I64 => Some(PrimitiveConstant::I64(1)),
        MirType::U64 => Some(PrimitiveConstant::U64(1)),
        MirType::U8 => Some(PrimitiveConstant::U8(1)),
        MirType::F64
        | MirType::Bool
        | MirType::Function(_)
        | MirType::Array(_)
        | MirType::Class(_)
        | MirType::Interface(_)
        | MirType::Obj
        | MirType::Shared(_)
        | MirType::Optional(_)
        | MirType::Unit => None,
    }
}

const fn all_ones(ty: MirType) -> Option<PrimitiveConstant> {
    match ty {
        MirType::I64 => Some(PrimitiveConstant::I64(-1)),
        MirType::U64 => Some(PrimitiveConstant::U64(u64::MAX)),
        MirType::U8 => Some(PrimitiveConstant::U8(u8::MAX)),
        MirType::F64
        | MirType::Bool
        | MirType::Function(_)
        | MirType::Array(_)
        | MirType::Class(_)
        | MirType::Interface(_)
        | MirType::Obj
        | MirType::Shared(_)
        | MirType::Optional(_)
        | MirType::Unit => None,
    }
}

fn is_zero(value: Option<PrimitiveConstant>, ty: MirType) -> bool {
    value.is_some_and(|value| Some(value) == zero(ty))
}

fn is_one(value: Option<PrimitiveConstant>, ty: MirType) -> bool {
    value.is_some_and(|value| Some(value) == one(ty))
}

fn is_all_ones(value: Option<PrimitiveConstant>, ty: MirType) -> bool {
    value.is_some_and(|value| Some(value) == all_ones(ty))
}

#[cfg(test)]
#[path = "primitive_algebra/tests.rs"]
mod tests;
