//! Exhaustive scalar-expression dispatch and primitive operations.

use super::*;
use crate::hir::{
    HirBinaryOperation, HirComparisonOperand, HirComparisonPredicate, HirExpression,
    HirExpressionKind, HirIntegerCast, HirIntegerType, HirPrimitiveComparison, HirUnaryOperation,
};

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
            HirExpressionKind::PrimitiveComparison {
                operation,
                left,
                right,
            } => self.lower_primitive_comparison(expression, *operation, left, right),
            HirExpressionKind::IntegerCast { operation, operand } => {
                self.lower_integer_cast(expression, *operation, operand)
            }
            HirExpressionKind::DirectCall {
                function,
                arguments,
            } => self.lower_direct_call(expression, *function, arguments),
            HirExpressionKind::StaticCall { method, arguments } => {
                self.lower_static_call(expression, *method, arguments)
            }
            HirExpressionKind::Grouped(inner) => self.lower_expression(inner),
            HirExpressionKind::FieldRead(place) => {
                let optional_mark = self.optional_view_mark();
                let place = self.lower_field_place(place);
                let result = self.assign(
                    MirRvalueKind::Load(place),
                    lower_type(expression.ty),
                    expression.span,
                );
                self.end_optional_views_from(optional_mark, expression.span);
                Some(result)
            }
            HirExpressionKind::MethodCall {
                receiver,
                target,
                arguments,
            } => self.lower_method_call(expression, receiver, *target, arguments),
            HirExpressionKind::InterfaceCall {
                receiver,
                target,
                arguments,
            } => self.lower_interface_call(expression, receiver, *target, arguments),
            HirExpressionKind::TypeTest(test) => Some(self.lower_type_test(expression, test)),
            HirExpressionKind::PresenceTest { source, kind } => {
                Some(self.lower_presence_test(expression, source, *kind))
            }
            HirExpressionKind::Unwrap(source) => {
                Some(self.lower_optional_unwrap(expression, source))
            }
            HirExpressionKind::ArrayLength(length) => Some(self.lower_array_length(length)),
            HirExpressionKind::ArrayElement(element)
                if matches!(
                    expression.ty,
                    crate::hir::Type::I64
                        | crate::hir::Type::U64
                        | crate::hir::Type::U8
                        | crate::hir::Type::F64
                        | crate::hir::Type::Bool
                ) =>
            {
                let place = self.lower_array_element_place(element);
                Some(self.assign(
                    MirRvalueKind::Load(place),
                    lower_type(expression.ty),
                    expression.span,
                ))
            }
            HirExpressionKind::ArrayConstruction(_)
            | HirExpressionKind::ArrayElement(_)
            | HirExpressionKind::ArraySlice(_) => None,
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
                    HirUnaryOperation::LogicalNotBool => MirUnaryOperation::LogicalNotBool,
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
        let (left, right) = self.lower_binary_operands(
            left,
            right,
            lower_type(operation.operand_type()),
            expression,
        );
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

    fn lower_primitive_comparison(
        &mut self,
        expression: &HirExpression,
        operation: HirPrimitiveComparison,
        left: &HirExpression,
        right: &HirExpression,
    ) -> Option<ValueId> {
        let operation = MirPrimitiveComparison {
            predicate: match operation.predicate {
                HirComparisonPredicate::Equal => MirComparisonPredicate::Equal,
                HirComparisonPredicate::NotEqual => MirComparisonPredicate::NotEqual,
                HirComparisonPredicate::LessThan => MirComparisonPredicate::LessThan,
                HirComparisonPredicate::LessEqual => MirComparisonPredicate::LessEqual,
                HirComparisonPredicate::GreaterThan => MirComparisonPredicate::GreaterThan,
                HirComparisonPredicate::GreaterEqual => MirComparisonPredicate::GreaterEqual,
            },
            operand: match operation.operand {
                HirComparisonOperand::Integer(integer) => {
                    MirComparisonOperand::Integer(lower_integer_type(integer))
                }
                HirComparisonOperand::Bool => MirComparisonOperand::Bool,
            },
        };
        let (left, right) =
            self.lower_binary_operands(left, right, operation.operand_type(), expression);
        Some(self.assign(
            MirRvalueKind::PrimitiveComparison {
                operation,
                left,
                right,
            },
            operation.result_type(),
            expression.span,
        ))
    }

    fn lower_integer_cast(
        &mut self,
        expression: &HirExpression,
        operation: HirIntegerCast,
        operand: &HirExpression,
    ) -> Option<ValueId> {
        let operand = self
            .lower_expression(operand)
            .expect("typed integer-cast operand must produce a value");
        let operation = MirIntegerCast {
            source: lower_integer_type(operation.source),
            target: lower_integer_type(operation.target),
        };
        Some(self.assign(
            MirRvalueKind::IntegerCast { operation, operand },
            operation.result_type(),
            expression.span,
        ))
    }

    fn lower_binary_operands(
        &mut self,
        left: &HirExpression,
        right: &HirExpression,
        operand_type: MirType,
        expression: &HirExpression,
    ) -> (ValueId, ValueId) {
        // This order is semantic: left is fully lowered before right.
        let left = self
            .lower_expression(left)
            .expect("typed binary operand must produce a value");
        let spilled_left = super::control_effect::expression_contains_control_effect(right)
            .then(|| self.spill_scalar(left, operand_type, expression.span));
        let right = self
            .lower_expression(right)
            .expect("typed binary operand must produce a value");
        let left = spilled_left
            .map(|(storage, ty)| {
                self.assign(MirRvalueKind::Load(storage.into()), ty, expression.span)
            })
            .unwrap_or(left);
        (left, right)
    }

    pub(super) fn spill_scalar(
        &mut self,
        value: ValueId,
        ty: MirType,
        span: crate::source::Span,
    ) -> (StorageId, MirType) {
        debug_assert!(ty.is_scalar_value());
        let storage = StorageId::new(self.input.callable, self.storage.len());
        self.storage.push(MirStorage {
            id: storage,
            source: None,
            name: format!("spill{}", storage.index()),
            kind: MirStorageKind::ScalarSpill,
            ty,
            span,
        });
        self.track_full_expression_storage(storage, span);
        self.emit(MirInstruction::Store(MirStore {
            destination: storage.into(),
            value,
            span,
        }));
        (storage, ty)
    }

    pub(super) fn assign(
        &mut self,
        kind: MirRvalueKind,
        ty: MirType,
        span: crate::source::Span,
    ) -> ValueId {
        let result = self.new_value(ty, span);
        self.emit(MirInstruction::Assign(MirAssignment {
            result,
            rvalue: MirRvalue { kind, ty },
            span,
        }));
        result
    }
}

const fn lower_integer_type(ty: HirIntegerType) -> MirIntegerType {
    match ty {
        HirIntegerType::I64 => MirIntegerType::I64,
        HirIntegerType::U64 => MirIntegerType::U64,
        HirIntegerType::U8 => MirIntegerType::U8,
    }
}
