//! Exact integer bitwise selection and diagnostics.

use super::*;
use crate::{
    diagnostics::format_type_list,
    hir::{
        HirBinaryOperation, HirExpressionKind, HirIntegerBitwiseOperation, HirIntegerType,
        HirUnaryOperation,
    },
    resolve::{ResolvedBinaryExpr, ResolvedBinaryOperator, ResolvedUnaryExpr},
};

const INTEGER_TYPE_NAMES: &[&str] = &["i64", "u64", "u8"];

impl CallableChecker<'_, '_> {
    pub(super) fn check_bitwise_complement(
        &mut self,
        unary: &ResolvedUnaryExpr,
    ) -> Option<HirExpression> {
        let actual = self.static_expression_type(&unary.operand);
        let Some(integer) = HirIntegerType::from_type(actual) else {
            self.diagnostics.push(
                Diagnostic::error(
                    TYPE_MISMATCH,
                    "bitwise complement requires a primitive integer operand",
                )
                .with_primary_label(
                    unary.operator_span,
                    "operator cannot be applied to this operand",
                )
                .with_secondary_label(
                    unary.operand.span(),
                    format!("operand has type `{}`", actual.name()),
                )
                .with_note(integer_type_note()),
            );
            return None;
        };

        let operand = self.check_expression(&unary.operand)?;
        let operation = HirUnaryOperation::BitwiseComplement(integer);
        Some(HirExpression {
            kind: HirExpressionKind::Unary {
                operation,
                operand: Box::new(operand),
            },
            ty: operation.result_type(),
            span: unary.span,
        })
    }

    pub(super) fn check_integer_bitwise_expression(
        &mut self,
        binary: &ResolvedBinaryExpr,
    ) -> Option<HirExpression> {
        let selected = bitwise_selection(binary.operator)
            .expect("bitwise checker must receive a bitwise operator");
        let left_type = self.static_expression_type(&binary.left);
        let right_type = self.static_expression_type(&binary.right);
        let integer = (left_type == right_type)
            .then(|| HirIntegerType::from_type(left_type))
            .flatten();
        let Some(integer) = integer else {
            self.diagnostics.push(
                Diagnostic::error(
                    TYPE_MISMATCH,
                    format!(
                        "bitwise `{}` requires operands of the same primitive integer type",
                        selected.spelling
                    ),
                )
                .with_primary_label(
                    binary.operator_span,
                    "operator cannot be applied to these operand types",
                )
                .with_secondary_label(
                    binary.left.span(),
                    format!("left operand has type `{}`", left_type.name()),
                )
                .with_secondary_label(
                    binary.right.span(),
                    format!("right operand has type `{}`", right_type.name()),
                )
                .with_note(integer_type_note()),
            );
            return None;
        };

        // Valid operands are checked exactly once in source order.
        let left = self.check_expression(&binary.left);
        let right = self.check_expression(&binary.right);
        let (left, right) = match (left, right) {
            (Some(left), Some(right)) => (left, right),
            _ => return None,
        };
        let operation = HirBinaryOperation::IntegerBitwise {
            operation: selected.operation,
            operand: integer,
        };
        Some(HirExpression {
            kind: HirExpressionKind::Binary {
                operation,
                left: Box::new(left),
                right: Box::new(right),
            },
            ty: operation.result_type(),
            span: binary.span,
        })
    }
}

fn integer_type_note() -> String {
    format!(
        "integer operand types are {}",
        format_type_list(INTEGER_TYPE_NAMES)
    )
}

#[derive(Clone, Copy)]
struct BitwiseSelection {
    operation: HirIntegerBitwiseOperation,
    spelling: &'static str,
}

const fn bitwise_selection(operator: ResolvedBinaryOperator) -> Option<BitwiseSelection> {
    match operator {
        ResolvedBinaryOperator::BitwiseAnd => Some(BitwiseSelection {
            operation: HirIntegerBitwiseOperation::And,
            spelling: "&",
        }),
        ResolvedBinaryOperator::BitwiseOr => Some(BitwiseSelection {
            operation: HirIntegerBitwiseOperation::Or,
            spelling: "|",
        }),
        ResolvedBinaryOperator::BitwiseXor => Some(BitwiseSelection {
            operation: HirIntegerBitwiseOperation::Xor,
            spelling: "^",
        }),
        ResolvedBinaryOperator::Add
        | ResolvedBinaryOperator::Subtract
        | ResolvedBinaryOperator::Multiply
        | ResolvedBinaryOperator::Divide
        | ResolvedBinaryOperator::Remainder
        | ResolvedBinaryOperator::ShiftLeft
        | ResolvedBinaryOperator::ShiftRight
        | ResolvedBinaryOperator::Equal
        | ResolvedBinaryOperator::NotEqual
        | ResolvedBinaryOperator::LessThan
        | ResolvedBinaryOperator::LessEqual
        | ResolvedBinaryOperator::GreaterThan
        | ResolvedBinaryOperator::GreaterEqual => None,
    }
}
