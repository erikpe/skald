//! Exact boolean selection for structured short-circuit expressions.

use crate::{
    diagnostics::Diagnostic,
    hir::{HirExpression, HirExpressionKind, HirLogicalExpression, HirLogicalOperation, Type},
    resolve::{ResolvedLogicalExpr, ResolvedLogicalOperator},
};

use super::CallableChecker;
use crate::typeck::TYPE_MISMATCH;

impl CallableChecker<'_, '_> {
    pub(super) fn check_logical_expression(
        &mut self,
        logical: &ResolvedLogicalExpr,
    ) -> Option<HirExpression> {
        let left_type = self.static_expression_type(&logical.left);
        let right_type = self.static_expression_type(&logical.right);

        // Check both operands exactly once and in source order even when one
        // fails. Independent operand diagnostics must not suppress the other
        // side or the exact-type diagnostic for this operator.
        let left = self.check_expression(&logical.left);
        let right = self.check_expression(&logical.right);

        let spelling = logical_operator_spelling(logical.operator);
        if left_type != Type::Bool || right_type != Type::Bool {
            self.diagnostics.push(
                Diagnostic::error(
                    TYPE_MISMATCH,
                    format!("logical operator `{spelling}` requires exact `bool` operands"),
                )
                .with_primary_label(
                    logical.operator_span,
                    "operator cannot be applied to these operand types",
                )
                .with_secondary_label(
                    logical.left.span(),
                    format!(
                        "left operand has type `{}`",
                        self.diagnostic_type_name(left_type)
                    ),
                )
                .with_secondary_label(
                    logical.right.span(),
                    format!(
                        "right operand has type `{}`",
                        self.diagnostic_type_name(right_type)
                    ),
                )
                .with_note(format!(
                    "`{spelling}` does not perform implicit conversion or truthiness testing"
                )),
            );
            return None;
        }

        let (left, right) = match (left, right) {
            (Some(left), Some(right)) => (left, right),
            _ => return None,
        };
        let operation = match logical.operator {
            ResolvedLogicalOperator::And => HirLogicalOperation::And,
            ResolvedLogicalOperator::Or => HirLogicalOperation::Or,
        };
        Some(HirExpression {
            kind: HirExpressionKind::Logical(Box::new(HirLogicalExpression::new(
                operation, left, right,
            ))),
            ty: operation.result_type(),
            span: logical.span,
        })
    }
}

const fn logical_operator_spelling(operator: ResolvedLogicalOperator) -> &'static str {
    match operator {
        ResolvedLogicalOperator::And => "&&",
        ResolvedLogicalOperator::Or => "||",
    }
}
