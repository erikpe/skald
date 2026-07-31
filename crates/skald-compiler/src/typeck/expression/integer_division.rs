//! Exact integer division/remainder selection and focused diagnostics.

use super::*;
use crate::{
    diagnostics::format_type_list,
    hir::{
        HirCheckedIntegerDivision, HirExpressionKind, HirIntegerDivisionKind,
        HirIntegerDivisionOperation, HirIntegerType,
    },
    resolve::{ResolvedBinaryExpr, ResolvedBinaryOperator},
};

const INTEGER_TYPE_NAMES: &[&str] = &["i64", "u64", "u8"];

impl CallableChecker<'_, '_> {
    pub(super) fn check_integer_division_expression(
        &mut self,
        binary: &ResolvedBinaryExpr,
    ) -> Option<HirExpression> {
        let (kind, spelling) = division_selection(binary.operator)
            .expect("integer-division checker must receive `/` or `%`");
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
                        "integer `{spelling}` requires operands of the same primitive integer type"
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
                .with_note(format!(
                    "integer operand types are {}",
                    format_type_list(INTEGER_TYPE_NAMES)
                )),
            );
            return None;
        };

        // Valid operands are checked exactly once in source order. The checked
        // HIR operation owns the failure capability from this boundary onward.
        let dividend = self.check_expression(&binary.left);
        let divisor = self.check_expression(&binary.right);
        let (dividend, divisor) = match (dividend, divisor) {
            (Some(dividend), Some(divisor)) => (dividend, divisor),
            _ => return None,
        };
        let operation = HirIntegerDivisionOperation {
            kind,
            operand: integer,
        };
        Some(HirExpression {
            kind: HirExpressionKind::CheckedIntegerDivision(Box::new(
                HirCheckedIntegerDivision::new(operation, dividend, divisor),
            )),
            ty: operation.result_type(),
            span: binary.span,
        })
    }
}

const fn division_selection(
    operator: ResolvedBinaryOperator,
) -> Option<(HirIntegerDivisionKind, &'static str)> {
    match operator {
        ResolvedBinaryOperator::Divide => Some((HirIntegerDivisionKind::Quotient, "/")),
        ResolvedBinaryOperator::Remainder => Some((HirIntegerDivisionKind::Remainder, "%")),
        ResolvedBinaryOperator::Add
        | ResolvedBinaryOperator::Subtract
        | ResolvedBinaryOperator::Multiply
        | ResolvedBinaryOperator::ShiftLeft
        | ResolvedBinaryOperator::ShiftRight
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
