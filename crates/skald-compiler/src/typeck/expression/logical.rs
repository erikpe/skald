//! Exact boolean selection and the source-feature completion gate.

use crate::{
    diagnostics::Diagnostic,
    hir::Type,
    resolve::{ResolvedLogicalExpr, ResolvedLogicalOperator},
};

use super::CallableChecker;
use crate::typeck::{LOGICAL_EXPRESSION_NOT_ENABLED, TYPE_MISMATCH};

impl CallableChecker<'_, '_> {
    pub(super) fn check_logical_expression(
        &mut self,
        logical: &ResolvedLogicalExpr,
    ) -> Option<crate::hir::HirExpression> {
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
                    format!("left operand has type `{}`", left_type.name()),
                )
                .with_secondary_label(
                    logical.right.span(),
                    format!("right operand has type `{}`", right_type.name()),
                )
                .with_note(format!(
                    "`{spelling}` does not perform implicit conversion or truthiness testing"
                )),
            );
            return None;
        }

        if left.is_none() || right.is_none() {
            return None;
        }

        self.diagnostics.push(
            Diagnostic::error(
                LOGICAL_EXPRESSION_NOT_ENABLED,
                format!(
                    "short-circuit logical operator `{spelling}` is not enabled for source programs"
                ),
            )
            .with_primary_label(
                logical.operator_span,
                "exact boolean operands were selected, but source lowering is not enabled",
            )
            .with_secondary_label(logical.left.span(), "left operand has type `bool`")
            .with_secondary_label(logical.right.span(), "right operand has type `bool`")
            .with_note(
                "logical source lowering remains behind the compiler completion gate until every \
                 valid operand and expression consumer is connected",
            ),
        );
        None
    }
}

const fn logical_operator_spelling(operator: ResolvedLogicalOperator) -> &'static str {
    match operator {
        ResolvedLogicalOperator::And => "&&",
        ResolvedLogicalOperator::Or => "||",
    }
}
