//! Call-scoped aliases to whole arrays and exact array elements.

use crate::{
    diagnostics::Diagnostic,
    hir::{
        HirArrayAliasArgument, HirArrayAliasSource, HirArrayAnchor, HirArrayReceiverSource,
        HirCallArgument, HirExpressionKind, HirOptionalOperand, HirOptionalStorage, Type,
    },
    resolve::ResolvedExpression,
    typeck::{
        expression::CallParameter,
        function::CallableChecker,
        program::{
            lower_parameter_mode, lower_type, INSUFFICIENT_ALIAS_ACCESS, INVALID_ALIAS_ARGUMENT,
            TYPE_MISMATCH,
        },
    },
};

use super::place::ArrayReceiverSyntax;

impl CallableChecker<'_, '_> {
    pub(in crate::typeck) fn check_array_alias_argument(
        &mut self,
        expression: &ResolvedExpression,
        parameter: &impl CallParameter,
    ) -> Option<HirCallArgument> {
        let expected = lower_type(self.program, parameter.type_syntax());
        let required = lower_parameter_mode(parameter.binding_mode())
            .required_access()
            .expect("array alias parameters require place access");

        let (source, actual, access, span) = if let (Type::Array(_), Some(unwrap)) =
            (expected, optional_array_alias_unwrap(expression))
        {
            let operand =
                self.require_optional_operand(&unwrap.source, unwrap.span, "checked array alias")?;
            let HirOptionalOperand::AggregatePlace(place) = &operand else {
                self.report_invalid_array_alias(expression, parameter);
                return None;
            };
            let actual = super::super::optional_types::payload_type(self.program, place.optional);
            let Type::Array(array) = actual else {
                self.report_invalid_array_alias(expression, parameter);
                return None;
            };
            let optional = place.optional;
            let access = self.optional_array_place_access(&place.storage, unwrap.span)?;
            (
                HirArrayAliasSource::OptionalPayload {
                    source: Box::new(operand),
                    optional,
                    array,
                },
                Type::Array(array),
                access,
                unwrap.span,
            )
        } else if matches!(expected, Type::Array(_)) && is_whole_array_alias_syntax(expression) {
            let mut receiver =
                self.check_array_receiver(expression, ArrayReceiverSyntax::Ordinary)?;
            if !is_aliasable_receiver(&receiver.source) {
                self.report_invalid_array_alias(expression, parameter);
                return None;
            }
            receiver.anchor = match receiver.source {
                HirArrayReceiverSource::Inline(_) => HirArrayAnchor::InlineBacking,
                HirArrayReceiverSource::Shared(_) => receiver.anchor,
            };
            if let Some(field) = direct_static_field_through_groups(expression) {
                receiver.access = self.static_field_alias_access(field);
            }
            let actual = Type::Array(receiver.array);
            let access = receiver.access;
            let span = receiver.span;
            (
                HirArrayAliasSource::Whole(Box::new(receiver)),
                actual,
                access,
                span,
            )
        } else {
            let checked = self.check_expression(expression)?;
            let actual = checked.ty;
            let span = checked.span;
            let Some(mut place) = array_element_through_groups(checked.kind) else {
                self.report_invalid_array_alias(expression, parameter);
                return None;
            };
            if place.receiver.ownership == crate::hir::HirArrayReceiverOwnership::Inline {
                place.receiver.anchor = HirArrayAnchor::InlineBacking;
            }
            let access = place.receiver.access;
            (HirArrayAliasSource::Element(place), actual, access, span)
        };

        if actual != expected {
            self.diagnostics.push(
                Diagnostic::error(
                    TYPE_MISMATCH,
                    format!(
                        "array alias source has type `{}`, but `{}` is required",
                        actual.name(),
                        expected.name()
                    ),
                )
                .with_primary_label(span, "this place has a different exact type")
                .with_secondary_label(parameter.type_syntax().span, "alias declared here"),
            );
            return None;
        }
        if !access.permits(required) {
            self.diagnostics.push(
                Diagnostic::error(
                    INSUFFICIENT_ALIAS_ACCESS,
                    "read-only array access cannot satisfy a mutable alias parameter",
                )
                .with_primary_label(span, "this array place is read-only")
                .with_secondary_label(parameter.span(), "mutable alias declared here"),
            );
            return None;
        }
        Some(HirCallArgument::ArrayAlias(HirArrayAliasArgument {
            source,
            target: expected,
            access: required,
            span,
        }))
    }

    fn optional_array_place_access(
        &mut self,
        storage: &HirOptionalStorage,
        span: crate::source::Span,
    ) -> Option<crate::hir::HirAccess> {
        match storage {
            HirOptionalStorage::Binding(binding) => self.binding_access(*binding, false, span),
            HirOptionalStorage::Static(place) => Some(self.static_field_alias_access(place.field)),
            HirOptionalStorage::Field(field) => Some(field.receiver.access()),
            HirOptionalStorage::ArrayElement(place) => Some(place.receiver.access),
            HirOptionalStorage::SharedPointee(_) => Some(crate::hir::HirAccess::ReadOnly),
        }
    }

    fn report_invalid_array_alias(
        &mut self,
        expression: &ResolvedExpression,
        parameter: &impl CallParameter,
    ) {
        self.diagnostics.push(
            Diagnostic::error(
                INVALID_ALIAS_ARGUMENT,
                "array alias argument must designate an existing array or exact element place",
            )
            .with_primary_label(
                expression.span(),
                "produced values and slices cannot be aliased",
            )
            .with_secondary_label(parameter.span(), "alias parameter declared here"),
        );
    }
}

fn array_element_through_groups(
    kind: HirExpressionKind,
) -> Option<Box<crate::hir::HirArrayElementPlace>> {
    match kind {
        HirExpressionKind::ArrayElement(place) => Some(place),
        HirExpressionKind::Grouped(inner) => array_element_through_groups(inner.kind),
        _ => None,
    }
}

fn is_whole_array_alias_syntax(expression: &ResolvedExpression) -> bool {
    match expression {
        ResolvedExpression::Binding(_)
        | ResolvedExpression::FieldAccess(_)
        | ResolvedExpression::StaticFieldAccess(_)
        | ResolvedExpression::Dereference(_) => true,
        ResolvedExpression::Grouped(grouped) => is_whole_array_alias_syntax(&grouped.expression),
        ResolvedExpression::ArrayProjection(_) => false,
        _ => false,
    }
}

fn is_aliasable_receiver(source: &HirArrayReceiverSource) -> bool {
    match source {
        HirArrayReceiverSource::Shared(_) => true,
        HirArrayReceiverSource::Inline(expression) => match &expression.kind {
            HirExpressionKind::Binding(_)
            | HirExpressionKind::FieldRead(_)
            | HirExpressionKind::StaticRead(_)
            | HirExpressionKind::ArrayElement(_) => true,
            HirExpressionKind::Grouped(inner) => {
                is_aliasable_receiver(&HirArrayReceiverSource::Inline(inner.clone()))
            }
            _ => false,
        },
    }
}

fn direct_static_field_through_groups(
    expression: &ResolvedExpression,
) -> Option<crate::identity::StaticFieldId> {
    match expression {
        ResolvedExpression::StaticFieldAccess(access) => Some(access.field),
        ResolvedExpression::Grouped(grouped) => {
            direct_static_field_through_groups(&grouped.expression)
        }
        _ => None,
    }
}

fn optional_array_alias_unwrap(
    expression: &ResolvedExpression,
) -> Option<&crate::resolve::ResolvedUnwrapExpr> {
    match expression {
        ResolvedExpression::Unwrap(unwrap) => Some(unwrap),
        ResolvedExpression::Grouped(grouped) => optional_array_alias_unwrap(&grouped.expression),
        _ => None,
    }
}
