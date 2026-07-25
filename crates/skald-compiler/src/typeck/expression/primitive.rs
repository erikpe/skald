//! Binding, literal, operator, grouping, and excluded-construction expressions.

use super::*;
use crate::{
    diagnostics::format_type_list,
    hir::{HirBinaryOperation, HirExpressionKind, HirUnaryOperation},
    resolve::{ResolvedBinaryOperator, ResolvedUnaryOperator},
};

use crate::typeck::{
    literal::{classify_i64_magnitude, i64_literal_through_groups, Magnitude},
    program::{INVALID_CONSTRUCTION, INVALID_OBJECT_CONTEXT},
};

const NUMERIC_TYPE_NAMES: &[&str] = &["i64", "u64", "u8", "f64"];
const NEGATABLE_TYPE_NAMES: &[&str] = &["i64", "f64"];

impl CallableChecker<'_, '_> {
    pub(super) fn check_binding_expression(
        &mut self,
        binding: &crate::resolve::ResolvedBindingExpr,
    ) -> Option<HirExpression> {
        let ty = self.binding_type(binding.binding);
        if matches!(ty, Type::Class(_) | Type::Obj | Type::Interface(_)) {
            let message = if matches!(ty, Type::Obj | Type::Interface(_)) {
                "an object view cannot be used as an ordinary value"
            } else {
                "an inline object cannot be used as an ordinary value"
            };
            self.diagnostics.push(
                Diagnostic::error(INVALID_OBJECT_CONTEXT, message).with_primary_label(
                    binding.span,
                    "use the object as a field or method receiver",
                ),
            );
            return None;
        }
        Some(HirExpression {
            kind: HirExpressionKind::Binding(binding.binding),
            ty,
            span: binding.span,
        })
    }

    pub(super) fn check_boolean_expression(
        &mut self,
        boolean: &crate::resolve::ResolvedBooleanExpr,
    ) -> Option<HirExpression> {
        Some(HirExpression {
            kind: HirExpressionKind::Boolean(boolean.value),
            ty: Type::Bool,
            span: boolean.span,
        })
    }

    pub(super) fn check_unary_expression(
        &mut self,
        unary: &crate::resolve::ResolvedUnaryExpr,
    ) -> Option<HirExpression> {
        if unary.operator == ResolvedUnaryOperator::Negate {
            if let Some(literal) = i64_literal_through_groups(&unary.operand) {
                match classify_i64_magnitude(&literal.spelling) {
                    Magnitude::MinimumBoundary => {
                        return Some(HirExpression {
                            kind: HirExpressionKind::I64(i64::MIN),
                            ty: Type::I64,
                            span: unary.span,
                        });
                    }
                    Magnitude::TooLarge => {
                        self.report_integer_out_of_range(
                            unary.span,
                            format!("-{}", literal.spelling),
                        );
                        return None;
                    }
                    Magnitude::PositiveI64 => {}
                }
            }
        }

        let operand = self.check_expression(&unary.operand)?;
        let operation = match operand.ty {
            Type::I64 => HirUnaryOperation::NegateI64,
            Type::F64 => HirUnaryOperation::NegateF64,
            _ => {
                self.diagnostics.push(
                    Diagnostic::error(
                        TYPE_MISMATCH,
                        format!(
                            "unary negation requires an {} operand",
                            format_type_list(NEGATABLE_TYPE_NAMES)
                        ),
                    )
                    .with_primary_label(
                        operand.span,
                        format!("operand has type `{}`", operand.ty.name()),
                    ),
                );
                return None;
            }
        };
        let ty = operand.ty;
        Some(HirExpression {
            kind: HirExpressionKind::Unary {
                operation,
                operand: Box::new(operand),
            },
            ty,
            span: unary.span,
        })
    }

    pub(super) fn check_binary_expression(
        &mut self,
        binary: &crate::resolve::ResolvedBinaryExpr,
    ) -> Option<HirExpression> {
        // Both operands are checked in source order so independent diagnostics accumulate.
        let left = self.check_expression(&binary.left);
        let right = self.check_expression(&binary.right);
        let (left, right) = match (left, right) {
            (Some(left), Some(right)) => (left, right),
            _ => return None,
        };
        let operation = (left.ty == right.ty)
            .then(|| select_binary_operation(binary.operator, left.ty))
            .flatten();
        let Some(operation) = operation else {
            self.diagnostics.push(
                Diagnostic::error(
                    TYPE_MISMATCH,
                    "binary arithmetic requires operands of the same numeric type",
                )
                .with_primary_label(
                    binary.operator_span,
                    "operator cannot be applied to these operand types",
                )
                .with_secondary_label(
                    left.span,
                    format!("left operand has type `{}`", left.ty.name()),
                )
                .with_secondary_label(
                    right.span,
                    format!("right operand has type `{}`", right.ty.name()),
                )
                .with_note(format!(
                    "numeric operand types are {}",
                    format_type_list(NUMERIC_TYPE_NAMES)
                )),
            );
            return None;
        };
        let ty = left.ty;
        Some(HirExpression {
            kind: HirExpressionKind::Binary {
                operation,
                left: Box::new(left),
                right: Box::new(right),
            },
            ty,
            span: binary.span,
        })
    }

    pub(super) fn check_grouped_expression(
        &mut self,
        grouped: &crate::resolve::ResolvedGroupedExpr,
    ) -> Option<HirExpression> {
        let inner = self.check_expression(&grouped.expression)?;
        let ty = inner.ty;
        Some(HirExpression {
            kind: HirExpressionKind::Grouped(Box::new(inner)),
            ty,
            span: grouped.span,
        })
    }

    pub(super) fn check_excluded_construction_expression(
        &mut self,
        construction: &crate::resolve::ResolvedConstructExpr,
    ) -> Option<HirExpression> {
        match &construction.mode {
            crate::resolve::ResolvedConstructionMode::Initialize { arguments } => {
                for argument in arguments {
                    let _ = self.check_expression(argument);
                }
            }
            crate::resolve::ResolvedConstructionMode::Copy { source, .. } => {
                let _ = self.check_expression(source);
            }
        }
        self.diagnostics.push(
            Diagnostic::error(
                INVALID_CONSTRUCTION,
                "construction is not allowed in this expression context",
            )
            .with_primary_label(
                construction.span,
                "use this object source in initialization, assignment, an object argument, or an object return",
            ),
        );
        None
    }
}

fn select_binary_operation(
    operator: ResolvedBinaryOperator,
    operand_type: Type,
) -> Option<HirBinaryOperation> {
    match (operator, operand_type) {
        (ResolvedBinaryOperator::Add, Type::I64) => Some(HirBinaryOperation::AddI64),
        (ResolvedBinaryOperator::Subtract, Type::I64) => Some(HirBinaryOperation::SubtractI64),
        (ResolvedBinaryOperator::Multiply, Type::I64) => Some(HirBinaryOperation::MultiplyI64),
        (ResolvedBinaryOperator::Add, Type::U64) => Some(HirBinaryOperation::AddU64),
        (ResolvedBinaryOperator::Subtract, Type::U64) => Some(HirBinaryOperation::SubtractU64),
        (ResolvedBinaryOperator::Multiply, Type::U64) => Some(HirBinaryOperation::MultiplyU64),
        (ResolvedBinaryOperator::Add, Type::U8) => Some(HirBinaryOperation::AddU8),
        (ResolvedBinaryOperator::Subtract, Type::U8) => Some(HirBinaryOperation::SubtractU8),
        (ResolvedBinaryOperator::Multiply, Type::U8) => Some(HirBinaryOperation::MultiplyU8),
        (ResolvedBinaryOperator::Add, Type::F64) => Some(HirBinaryOperation::AddF64),
        (ResolvedBinaryOperator::Subtract, Type::F64) => Some(HirBinaryOperation::SubtractF64),
        (ResolvedBinaryOperator::Multiply, Type::F64) => Some(HirBinaryOperation::MultiplyF64),
        (
            _,
            Type::Bool
            | Type::Unit
            | Type::Obj
            | Type::Class(_)
            | Type::Interface(_)
            | Type::Shared(_)
            | Type::OptionalShared(_)
            | Type::OptionalPrimitive(_)
            | Type::OptionalClass(_),
        ) => None,
    }
}
