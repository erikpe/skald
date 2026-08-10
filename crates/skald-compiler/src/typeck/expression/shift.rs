//! Checked integer-shift selection and focused operand diagnostics.

use super::*;
use crate::{
    diagnostics::format_type_list,
    hir::{
        HirCheckedShift, HirExpressionKind, HirIntegerType, HirShiftDirection, HirShiftOperation,
    },
    resolve::{ResolvedBinaryExpr, ResolvedBinaryOperator},
};

const LEFT_TYPE_NAMES: &[&str] = &["i64", "u64", "u8"];

impl CallableChecker<'_, '_> {
    pub(super) fn check_shift_expression(
        &mut self,
        binary: &ResolvedBinaryExpr,
    ) -> Option<HirExpression> {
        let (direction, spelling) =
            shift_selection(binary.operator).expect("shift checker must receive a shift operator");
        let left_type = self.static_expression_type(&binary.left);
        let count_type = self.static_expression_type(&binary.right);
        let Some(left) = HirIntegerType::from_type(left_type).filter(|_| count_type == Type::U64)
        else {
            self.diagnostics.push(
                Diagnostic::error(
                    TYPE_MISMATCH,
                    format!(
                        "shift `{spelling}` requires a primitive integer left operand and a `u64` count"
                    ),
                )
                .with_primary_label(
                    binary.operator_span,
                    "operator cannot be applied to these operand types",
                )
                .with_secondary_label(
                    binary.left.span(),
                    format!("left operand has type `{}`", self.diagnostic_type_name(left_type)),
                )
                .with_secondary_label(
                    binary.right.span(),
                    format!("count operand has type `{}`", self.diagnostic_type_name(count_type)),
                )
                .with_note(format!(
                    "left operand types are {}; the count type is exactly `u64`",
                    format_type_list(LEFT_TYPE_NAMES)
                )),
            );
            return None;
        };

        // Valid operands are checked exactly once in source order. The HIR
        // operation owns the mixed left/count type relationship thereafter.
        let left_expression = self.check_expression(&binary.left);
        let count_expression = self.check_expression(&binary.right);
        let (left_expression, count_expression) = match (left_expression, count_expression) {
            (Some(left_expression), Some(count_expression)) => (left_expression, count_expression),
            _ => return None,
        };
        let operation = HirShiftOperation { direction, left };
        Some(HirExpression {
            kind: HirExpressionKind::CheckedShift(Box::new(HirCheckedShift::new(
                operation,
                left_expression,
                count_expression,
            ))),
            ty: operation.result_type(),
            span: binary.span,
        })
    }
}

const fn shift_selection(
    operator: ResolvedBinaryOperator,
) -> Option<(HirShiftDirection, &'static str)> {
    match operator {
        ResolvedBinaryOperator::ShiftLeft => Some((HirShiftDirection::Left, "<<")),
        ResolvedBinaryOperator::ShiftRight => Some((HirShiftDirection::Right, ">>")),
        ResolvedBinaryOperator::Add
        | ResolvedBinaryOperator::Subtract
        | ResolvedBinaryOperator::Multiply
        | ResolvedBinaryOperator::Divide
        | ResolvedBinaryOperator::Remainder
        | ResolvedBinaryOperator::BitwiseAnd
        | ResolvedBinaryOperator::BitwiseOr
        | ResolvedBinaryOperator::BitwiseXor
        | ResolvedBinaryOperator::Equal
        | ResolvedBinaryOperator::NotEqual
        | ResolvedBinaryOperator::LessThan
        | ResolvedBinaryOperator::LessEqual
        | ResolvedBinaryOperator::GreaterThan
        | ResolvedBinaryOperator::GreaterEqual => None,
    }
}
