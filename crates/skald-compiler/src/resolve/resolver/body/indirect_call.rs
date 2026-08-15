//! Function-typed callee selection and grouped-call diagnostics.

use super::*;

impl CallableResolver<'_, '_> {
    pub(super) fn resolve_indirect_callee(
        &mut self,
        callee: &syntax::Expression,
    ) -> Option<(ResolvedExpression, FunctionTypeId)> {
        let callee = self.resolve_expression(callee)?;
        let Some(ResolvedTypeKind::Function(function_type)) =
            self.resolved_expression_type(&callee)
        else {
            self.diagnostics.push(
                Diagnostic::error(INVALID_CALL_TARGET, "expression is not callable")
                    .with_primary_label(callee.span(), "this expression has no function type"),
            );
            return None;
        };
        Some((callee, function_type))
    }

    pub(super) fn is_grouped_function_value_cast(&self, cast: &syntax::ObjectCastExpr) -> bool {
        matches!(cast.target_mode, syntax::ObjectCastTargetMode::Plain)
            && cast.target.arguments.is_none()
            && !cast.target.name.is_qualified()
            && self
                .lookup_binding(&cast.target.name.text)
                .is_some_and(|binding| matches!(binding.ty, ResolvedTypeKind::Function(_)))
    }

    pub(super) fn report_grouped_function_value_call(&mut self, cast: &syntax::ObjectCastExpr) {
        self.diagnostics.push(
            Diagnostic::error(
                INVALID_CALL_TARGET,
                "parenthesized function-value calls are not supported",
            )
            .with_primary_label(
                cast.target.span,
                "this parses as an object-cast target, not a grouped callee",
            )
            .with_note(format!(
                "call the function value without grouping: `{}(...)`",
                cast.target.name.text
            )),
        );
    }

    pub(super) fn report_parenthesized_call_target(&mut self, span: Span) {
        self.diagnostics.push(
            Diagnostic::error(
                INVALID_CALL_TARGET,
                "parenthesized call targets are not supported",
            )
            .with_primary_label(
                span,
                "remove the grouping or call an unambiguous postfix expression",
            ),
        );
    }
}
