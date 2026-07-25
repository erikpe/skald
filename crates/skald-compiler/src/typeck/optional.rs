//! Primitive optional construction and checked inspection.

use crate::{
    diagnostics::Diagnostic,
    hir::{
        HirExpression, HirExpressionKind, HirOptionalOperand, HirOptionalPlace, HirOptionalSource,
        HirOptionalStorage, HirPresenceTestKind, HirPrimitiveType, Type,
    },
    resolve::{
        ResolvedExpression, ResolvedPresenceTestExpr, ResolvedPresenceTestKind, ResolvedUnwrapExpr,
    },
};

use super::{
    expression::is_call_through_groups, function::CallableChecker, program::TYPE_MISMATCH,
};

impl CallableChecker<'_, '_> {
    pub(super) fn check_optional_source(
        &mut self,
        source: &ResolvedExpression,
        payload: HirPrimitiveType,
        context: &'static str,
    ) -> Option<HirOptionalSource> {
        if let ResolvedExpression::Absent(absent) = source {
            return Some(HirOptionalSource::Absent { span: absent.span });
        }
        if let Some(place) = self.optional_place(source) {
            if place.payload == payload {
                return Some(HirOptionalSource::Copy(place));
            }
            self.report_optional_payload_mismatch(place.payload, payload, place.span, context);
            return None;
        }

        let value = self.check_expression(source)?;
        if value.ty == Type::OptionalPrimitive(payload) {
            return Some(HirOptionalSource::Produced(Box::new(value)));
        }
        if value.ty != payload.payload_type() {
            self.diagnostics.push(
                Diagnostic::error(
                    TYPE_MISMATCH,
                    format!(
                        "{context} requires `{}` or `{}?`",
                        payload.name(),
                        payload.name()
                    ),
                )
                .with_primary_label(value.span, format!("source has type `{}`", value.ty.name())),
            );
            return None;
        }
        Some(HirOptionalSource::Present(value))
    }

    pub(super) fn check_presence_test(
        &mut self,
        test: &ResolvedPresenceTestExpr,
    ) -> Option<HirExpression> {
        let source = self.require_optional_operand(&test.source, test.span, "presence test")?;
        Some(HirExpression {
            kind: HirExpressionKind::PresenceTest {
                source,
                kind: match test.kind {
                    ResolvedPresenceTestKind::Some => HirPresenceTestKind::Some,
                    ResolvedPresenceTestKind::None => HirPresenceTestKind::None,
                },
            },
            ty: Type::Bool,
            span: test.span,
        })
    }

    pub(super) fn check_optional_unwrap(
        &mut self,
        unwrap: &ResolvedUnwrapExpr,
    ) -> Option<HirExpression> {
        let source =
            self.require_optional_operand(&unwrap.source, unwrap.span, "checked unwrap")?;
        let payload = source.payload();
        Some(HirExpression {
            kind: HirExpressionKind::Unwrap(source),
            ty: payload.payload_type(),
            span: unwrap.span,
        })
    }

    fn require_optional_operand(
        &mut self,
        expression: &ResolvedExpression,
        span: crate::source::Span,
        context: &'static str,
    ) -> Option<HirOptionalOperand> {
        if let Some(place) = self.optional_place(expression) {
            return Some(HirOptionalOperand::Place(place));
        }
        if is_call_through_groups(expression) {
            if let Some(value) = self.check_expression(expression) {
                if matches!(value.ty, Type::OptionalPrimitive(_)) {
                    return Some(HirOptionalOperand::Produced(Box::new(value)));
                }
                self.report_non_optional_operand(value.ty, span, context);
                return None;
            }
        }
        let actual = self.check_expression(expression).map(|value| value.ty);
        let label = actual.map_or_else(
            || "expected a primitive optional local".to_owned(),
            |ty| format!("expression has non-optional type `{}`", ty.name()),
        );
        self.diagnostics.push(
            Diagnostic::error(
                TYPE_MISMATCH,
                format!("{context} requires a primitive optional value"),
            )
            .with_primary_label(span, label),
        );
        None
    }

    pub(super) fn optional_place(
        &mut self,
        expression: &ResolvedExpression,
    ) -> Option<HirOptionalPlace> {
        match expression {
            ResolvedExpression::Binding(binding) => {
                let Type::OptionalPrimitive(payload) = self.binding_type(binding.binding) else {
                    return None;
                };
                Some(HirOptionalPlace {
                    storage: HirOptionalStorage::Binding(binding.binding),
                    payload,
                    span: binding.span,
                })
            }
            ResolvedExpression::Grouped(grouped) => {
                self.optional_place(&grouped.expression)
                    .map(|place| HirOptionalPlace {
                        span: grouped.span,
                        ..place
                    })
            }
            ResolvedExpression::FieldAccess(access) => {
                let expression = self.check_field_read(access)?;
                let Type::OptionalPrimitive(payload) = expression.ty else {
                    return None;
                };
                let HirExpressionKind::FieldRead(place) = expression.kind else {
                    unreachable!("field checking must produce a field-read expression");
                };
                Some(HirOptionalPlace {
                    storage: HirOptionalStorage::Field(place),
                    payload,
                    span: expression.span,
                })
            }
            _ => None,
        }
    }

    fn report_non_optional_operand(
        &mut self,
        actual: Type,
        span: crate::source::Span,
        context: &'static str,
    ) {
        self.diagnostics.push(
            Diagnostic::error(
                TYPE_MISMATCH,
                format!("{context} requires a primitive optional value"),
            )
            .with_primary_label(
                span,
                format!("expression has non-optional type `{}`", actual.name()),
            ),
        );
    }

    fn report_optional_payload_mismatch(
        &mut self,
        actual: HirPrimitiveType,
        expected: HirPrimitiveType,
        span: crate::source::Span,
        context: &'static str,
    ) {
        self.diagnostics.push(
            Diagnostic::error(
                TYPE_MISMATCH,
                format!("{context} requires `{}?`", expected.name()),
            )
            .with_primary_label(span, format!("source has type `{}?`", actual.name())),
        );
    }
}
