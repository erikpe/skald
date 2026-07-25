//! Primitive optional construction and checked inspection.

use crate::{
    diagnostics::Diagnostic,
    hir::{
        HirExpression, HirExpressionKind, HirOptionalPlace, HirOptionalSource, HirPresenceTestKind,
        HirPrimitiveType, Type,
    },
    resolve::{
        ResolvedExpression, ResolvedPresenceTestExpr, ResolvedPresenceTestKind, ResolvedUnwrapExpr,
    },
};

use super::{function::CallableChecker, program::TYPE_MISMATCH};

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
        let source = self.require_optional_place(&test.source, test.span, "presence test")?;
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
        let source = self.require_optional_place(&unwrap.source, unwrap.span, "checked unwrap")?;
        Some(HirExpression {
            kind: HirExpressionKind::Unwrap(source),
            ty: source.payload.payload_type(),
            span: unwrap.span,
        })
    }

    fn require_optional_place(
        &mut self,
        expression: &ResolvedExpression,
        span: crate::source::Span,
        context: &'static str,
    ) -> Option<HirOptionalPlace> {
        if let Some(place) = self.optional_place(expression) {
            return Some(place);
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

    fn optional_place(&self, expression: &ResolvedExpression) -> Option<HirOptionalPlace> {
        match expression {
            ResolvedExpression::Binding(binding) => {
                let Type::OptionalPrimitive(payload) = self.binding_type(binding.binding) else {
                    return None;
                };
                Some(HirOptionalPlace {
                    binding: binding.binding,
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
            _ => None,
        }
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
