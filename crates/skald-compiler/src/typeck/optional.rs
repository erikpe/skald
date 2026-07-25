//! Primitive optional construction and checked inspection.

use crate::{
    diagnostics::Diagnostic,
    hir::{
        HirCheckedOptionalView, HirClassOptionalAssignment, HirClassOptionalInitialize,
        HirClassOptionalPlace, HirClassOptionalSource, HirExpression, HirExpressionKind,
        HirOptionalOperand, HirOptionalPlace, HirOptionalSource, HirOptionalStorage,
        HirPresenceTestKind, HirPrimitiveType, Type,
    },
    resolve::{
        ResolvedExpression, ResolvedPresenceTestExpr, ResolvedPresenceTestKind, ResolvedUnwrapExpr,
    },
};

use super::{
    expression::is_call_through_groups, function::CallableChecker, program::TYPE_MISMATCH,
};

impl CallableChecker<'_, '_> {
    pub(super) fn check_class_optional_initialize(
        &mut self,
        class: crate::identity::ClassId,
        source: &ResolvedExpression,
        context: &'static str,
    ) -> Option<HirClassOptionalInitialize> {
        let checked = self.check_class_optional_source(source, class, context)?;
        let copy_constructor = if matches!(checked, HirClassOptionalSource::Absent { .. }) {
            None
        } else {
            let Some(operation) = self.copy_capabilities.constructor(class).selected() else {
                self.report_unavailable_copy_operation(class, true, source.span());
                return None;
            };
            Some(operation)
        };
        Some(HirClassOptionalInitialize {
            class,
            source: checked,
            copy_constructor,
            span: source.span(),
        })
    }

    pub(super) fn check_class_optional_assignment(
        &mut self,
        destination: HirClassOptionalPlace,
        source: &ResolvedExpression,
        context: &'static str,
    ) -> Option<HirClassOptionalAssignment> {
        let checked = self.check_class_optional_source(source, destination.class, context)?;
        let (copy_constructor, copy_assignment) =
            if matches!(checked, HirClassOptionalSource::Absent { .. }) {
                (None, None)
            } else {
                let Some(construction) = self
                    .copy_capabilities
                    .constructor(destination.class)
                    .selected()
                else {
                    self.report_unavailable_copy_operation(destination.class, true, source.span());
                    return None;
                };
                let Some(assignment) = self
                    .copy_capabilities
                    .assignment(destination.class)
                    .selected()
                else {
                    self.report_unavailable_copy_operation(destination.class, false, source.span());
                    return None;
                };
                (Some(construction), Some(assignment))
            };
        Some(HirClassOptionalAssignment {
            destination,
            source: checked,
            copy_constructor,
            copy_assignment,
            kind: crate::hir::HirOptionalWriteKind::Assign,
            span: source.span(),
        })
    }

    fn check_class_optional_source(
        &mut self,
        source: &ResolvedExpression,
        class: crate::identity::ClassId,
        context: &'static str,
    ) -> Option<HirClassOptionalSource> {
        if let ResolvedExpression::Absent(absent) = source {
            return Some(HirClassOptionalSource::Absent { span: absent.span });
        }
        if let Some(place) = self.class_optional_place(source) {
            if place.class == class {
                return Some(HirClassOptionalSource::Copy(place));
            }
            self.diagnostics.push(
                Diagnostic::error(
                    TYPE_MISMATCH,
                    format!("{context} requires `class {class}?`"),
                )
                .with_primary_label(
                    place.span,
                    format!("source has type `class {}?`", place.class),
                ),
            );
            return None;
        }
        if is_call_through_groups(source) {
            let expression = self.check_expression(source)?;
            if expression.ty == Type::OptionalClass(class) {
                return Some(HirClassOptionalSource::Produced(Box::new(expression)));
            }
            if let Type::OptionalClass(actual) = expression.ty {
                self.diagnostics.push(
                    Diagnostic::error(
                        TYPE_MISMATCH,
                        format!("{context} requires `class {class}?`"),
                    )
                    .with_primary_label(
                        expression.span,
                        format!("source has type `class {actual}?`"),
                    ),
                );
                return None;
            }
            if expression.ty == Type::Class(class) {
                return Some(HirClassOptionalSource::Present(
                    crate::hir::HirObjectSource::Produced(crate::hir::HirObjectProducer::Call(
                        super::function::lower_object_call(expression, class),
                    )),
                ));
            }
            self.diagnostics.push(
                Diagnostic::error(
                    TYPE_MISMATCH,
                    format!("{context} requires `class {class}` or `class {class}?`"),
                )
                .with_primary_label(
                    expression.span,
                    format!("source has type `{}`", expression.ty.name()),
                ),
            );
            return None;
        }
        self.check_object_source(source, class, context)
            .map(HirClassOptionalSource::Present)
    }

    pub(super) fn class_optional_place(
        &mut self,
        expression: &ResolvedExpression,
    ) -> Option<HirClassOptionalPlace> {
        match expression {
            ResolvedExpression::Binding(binding) => {
                let Type::OptionalClass(class) = self.binding_type(binding.binding) else {
                    return None;
                };
                Some(HirClassOptionalPlace {
                    storage: HirOptionalStorage::Binding(binding.binding),
                    class,
                    span: binding.span,
                })
            }
            ResolvedExpression::Grouped(grouped) => self
                .class_optional_place(&grouped.expression)
                .map(|place| HirClassOptionalPlace {
                    span: grouped.span,
                    ..place
                }),
            ResolvedExpression::FieldAccess(access) => {
                let expression = self.check_field_read(access)?;
                let Type::OptionalClass(class) = expression.ty else {
                    return None;
                };
                let HirExpressionKind::FieldRead(place) = expression.kind else {
                    unreachable!("field checking must produce a field-read expression");
                };
                Some(HirClassOptionalPlace {
                    storage: HirOptionalStorage::Field(place),
                    class,
                    span: expression.span,
                })
            }
            _ => None,
        }
    }

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
        if matches!(
            source,
            HirOptionalOperand::ClassPlace(_) | HirOptionalOperand::ClassProduced(_)
        ) {
            self.diagnostics.push(
                Diagnostic::error(
                    crate::typeck::program::INVALID_OBJECT_CONTEXT,
                    "an inline class payload is an object place, not a scalar value",
                )
                .with_primary_label(
                    unwrap.span,
                    "consume this checked payload as a member receiver, alias, cast, type test, or copy source",
                ),
            );
            return None;
        }
        let payload = source.payload();
        Some(HirExpression {
            kind: HirExpressionKind::Unwrap(source),
            ty: payload.payload_type(),
            span: unwrap.span,
        })
    }

    pub(super) fn check_class_optional_view(
        &mut self,
        unwrap: &ResolvedUnwrapExpr,
    ) -> Option<HirCheckedOptionalView> {
        let source =
            self.require_optional_operand(&unwrap.source, unwrap.span, "checked unwrap")?;
        let access = match &source {
            HirOptionalOperand::ClassPlace(place) => match &place.storage {
                HirOptionalStorage::Binding(binding) => {
                    self.binding_access(*binding, false, unwrap.span)?
                }
                HirOptionalStorage::Field(field) => field.receiver.access,
            },
            HirOptionalOperand::ClassProduced(_) => crate::hir::HirAccess::Mutable,
            HirOptionalOperand::Place(_) | HirOptionalOperand::Produced(_) => {
                self.diagnostics.push(
                    Diagnostic::error(
                        crate::typeck::program::INVALID_OBJECT_CONTEXT,
                        "checked object view requires an inline class optional",
                    )
                    .with_primary_label(unwrap.span, "this optional has a primitive payload"),
                );
                return None;
            }
        };
        Some(HirCheckedOptionalView {
            source,
            access,
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
        if let Some(place) = self.class_optional_place(expression) {
            return Some(HirOptionalOperand::ClassPlace(place));
        }
        if is_call_through_groups(expression) {
            if let Some(value) = self.check_expression(expression) {
                if matches!(
                    value.ty,
                    Type::OptionalPrimitive(_) | Type::OptionalClass(_)
                ) {
                    return Some(match value.ty {
                        Type::OptionalPrimitive(_) => HirOptionalOperand::Produced(Box::new(value)),
                        Type::OptionalClass(_) => {
                            HirOptionalOperand::ClassProduced(Box::new(value))
                        }
                        _ => unreachable!(),
                    });
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
