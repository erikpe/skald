//! Exhaustive scalar-expression dispatch and primitive operations.

use super::*;
use crate::hir::{HirBinaryOperation, HirExpression, HirExpressionKind, HirUnaryOperation};

impl BodyLowerer<'_> {
    pub(super) fn lower_expression(&mut self, expression: &HirExpression) -> Option<ValueId> {
        match &expression.kind {
            HirExpressionKind::Binding(binding) => {
                let storage = self.storage_for_binding(*binding);
                Some(self.assign(
                    MirRvalueKind::Load(storage.into()),
                    lower_type(expression.ty),
                    expression.span,
                ))
            }
            HirExpressionKind::I64(value) => Some(self.assign(
                MirRvalueKind::ConstantI64(*value),
                lower_type(expression.ty),
                expression.span,
            )),
            HirExpressionKind::U64(value) => Some(self.assign(
                MirRvalueKind::ConstantU64(*value),
                lower_type(expression.ty),
                expression.span,
            )),
            HirExpressionKind::U8(value) => Some(self.assign(
                MirRvalueKind::ConstantU8(*value),
                lower_type(expression.ty),
                expression.span,
            )),
            HirExpressionKind::F64Bits(bits) => Some(self.assign(
                MirRvalueKind::ConstantF64Bits(*bits),
                lower_type(expression.ty),
                expression.span,
            )),
            HirExpressionKind::Boolean(value) => Some(self.assign(
                MirRvalueKind::ConstantBool(*value),
                lower_type(expression.ty),
                expression.span,
            )),
            HirExpressionKind::Unary { operation, operand } => {
                self.lower_unary(expression, *operation, operand)
            }
            HirExpressionKind::Binary {
                operation,
                left,
                right,
            } => self.lower_binary(expression, *operation, left, right),
            HirExpressionKind::DirectCall {
                function,
                arguments,
            } => self.lower_direct_call(expression, *function, arguments),
            HirExpressionKind::Grouped(inner) => self.lower_expression(inner),
            HirExpressionKind::FieldRead(place) => Some(self.assign(
                MirRvalueKind::Load(self.lower_field_place(place)),
                lower_type(expression.ty),
                expression.span,
            )),
            HirExpressionKind::MethodCall {
                receiver,
                target,
                arguments,
            } => self.lower_method_call(expression, receiver, *target, arguments),
        }
    }

    fn lower_unary(
        &mut self,
        expression: &HirExpression,
        operation: HirUnaryOperation,
        operand: &HirExpression,
    ) -> Option<ValueId> {
        let operand = self
            .lower_expression(operand)
            .expect("typed unary operand must produce a value");
        Some(self.assign(
            MirRvalueKind::Unary {
                operation: match operation {
                    HirUnaryOperation::NegateI64 => MirUnaryOperation::NegateI64,
                    HirUnaryOperation::NegateF64 => MirUnaryOperation::NegateF64,
                },
                operand,
            },
            lower_type(expression.ty),
            expression.span,
        ))
    }

    fn lower_binary(
        &mut self,
        expression: &HirExpression,
        operation: HirBinaryOperation,
        left: &HirExpression,
        right: &HirExpression,
    ) -> Option<ValueId> {
        // This order is semantic: left is fully lowered before right.
        let left = self
            .lower_expression(left)
            .expect("typed binary operand must produce a value");
        let right = self
            .lower_expression(right)
            .expect("typed binary operand must produce a value");
        Some(self.assign(
            MirRvalueKind::Binary {
                operation: match operation {
                    HirBinaryOperation::AddI64 => MirBinaryOperation::AddI64,
                    HirBinaryOperation::SubtractI64 => MirBinaryOperation::SubtractI64,
                    HirBinaryOperation::MultiplyI64 => MirBinaryOperation::MultiplyI64,
                    HirBinaryOperation::AddU64 => MirBinaryOperation::AddU64,
                    HirBinaryOperation::SubtractU64 => MirBinaryOperation::SubtractU64,
                    HirBinaryOperation::MultiplyU64 => MirBinaryOperation::MultiplyU64,
                    HirBinaryOperation::AddU8 => MirBinaryOperation::AddU8,
                    HirBinaryOperation::SubtractU8 => MirBinaryOperation::SubtractU8,
                    HirBinaryOperation::MultiplyU8 => MirBinaryOperation::MultiplyU8,
                    HirBinaryOperation::AddF64 => MirBinaryOperation::AddF64,
                    HirBinaryOperation::SubtractF64 => MirBinaryOperation::SubtractF64,
                    HirBinaryOperation::MultiplyF64 => MirBinaryOperation::MultiplyF64,
                },
                left,
                right,
            },
            lower_type(expression.ty),
            expression.span,
        ))
    }

    fn assign(&mut self, kind: MirRvalueKind, ty: MirType, span: crate::source::Span) -> ValueId {
        let result = self.new_value(ty, span);
        self.emit(MirInstruction::Assign(MirAssignment {
            result,
            rvalue: MirRvalue { kind, ty },
            span,
        }));
        result
    }
}
