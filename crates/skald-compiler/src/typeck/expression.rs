//! Expression, call, binding, and primitive-operation checking.

use crate::{
    diagnostics::{Diagnostic, Diagnostics},
    hir::{HirExpression, Type},
    resolve::ResolvedExpression,
    source::Span,
};

use super::{function::CallableChecker, program::TYPE_MISMATCH};

mod alias;
mod call;
mod place;
mod primitive;

pub(super) use place::ObjectPlaceUse;

impl CallableChecker<'_, '_> {
    pub(super) fn check_expression(
        &mut self,
        expression: &ResolvedExpression,
    ) -> Option<HirExpression> {
        match expression {
            ResolvedExpression::Binding(binding) => self.check_binding_expression(binding),
            ResolvedExpression::NumericLiteral(literal) => self.check_numeric_literal(literal),
            ResolvedExpression::Boolean(boolean) => self.check_boolean_expression(boolean),
            ResolvedExpression::Unary(unary) => self.check_unary_expression(unary),
            ResolvedExpression::Binary(binary) => self.check_binary_expression(binary),
            ResolvedExpression::DirectCall(call) => self.check_direct_call(call),
            ResolvedExpression::Grouped(grouped) => self.check_grouped_expression(grouped),
            ResolvedExpression::FieldAccess(access) => self.check_field_read(access),
            ResolvedExpression::MethodCall(call) => self.check_method_call(call),
            ResolvedExpression::Construct(construction) => {
                self.check_excluded_construction_expression(construction)
            }
        }
    }
}

pub(super) fn require_type(
    actual: Type,
    expected: Type,
    span: Span,
    context: &'static str,
    diagnostics: &mut Diagnostics,
) -> bool {
    if actual == expected {
        return true;
    }
    diagnostics.push(
        Diagnostic::error(
            TYPE_MISMATCH,
            format!(
                "{context} has type `{}` but `{}` is required",
                actual.name(),
                expected.name()
            ),
        )
        .with_primary_label(span, "type mismatch"),
    );
    false
}

pub(super) fn is_call_through_groups(expression: &ResolvedExpression) -> bool {
    match expression {
        ResolvedExpression::DirectCall(_) | ResolvedExpression::MethodCall(_) => true,
        ResolvedExpression::Grouped(grouped) => is_call_through_groups(&grouped.expression),
        _ => false,
    }
}
