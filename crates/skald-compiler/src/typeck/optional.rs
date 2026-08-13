//! Primitive optional construction and checked inspection.

use crate::{
    diagnostics::Diagnostic,
    hir::{
        HirCheckedOptionalView, HirClassOptionalAssignment, HirClassOptionalInitialize,
        HirClassOptionalPlace, HirClassOptionalSource, HirExpression, HirExpressionKind,
        HirOptionalOperand, HirOptionalPlace, HirOptionalSharedAssignment,
        HirOptionalSharedInitialize, HirOptionalSharedPlace, HirOptionalSharedSource,
        HirOptionalSource, HirOptionalStorage, HirPresenceTestKind, HirPrimitiveType,
        HirSharedTarget, Type,
    },
    resolve::{
        ResolvedExpression, ResolvedPresenceTestExpr, ResolvedPresenceTestKind, ResolvedUnwrapExpr,
    },
};

use super::{
    expression::is_call_through_groups, function::CallableChecker,
    optional_types::OptionalPayloadKind, program::TYPE_MISMATCH,
};

impl CallableChecker<'_, '_> {
    pub(super) fn check_optional_value(
        &mut self,
        optional: crate::identity::OptionalTypeId,
        source: &ResolvedExpression,
        context: &'static str,
    ) -> Option<crate::hir::HirOptionalValue> {
        let span = source.span();
        if matches!(source, ResolvedExpression::Absent(_)) {
            return Some(crate::hir::HirOptionalValue {
                optional,
                source: crate::hir::HirOptionalValueSource::Absent,
                span,
            });
        }

        let payload = super::optional_types::payload_type(self.program, optional);
        if let ResolvedExpression::Present(present) = source {
            let value =
                self.check_stored_value_initialization(payload, &present.value, "`some` payload")?;
            return Some(crate::hir::HirOptionalValue {
                optional,
                source: crate::hir::HirOptionalValueSource::Present(Box::new(value)),
                span,
            });
        }

        let actual = self.static_expression_type(source);
        if actual == Type::Optional(optional) {
            if let Some(place) = self.optional_value_place(source) {
                return Some(crate::hir::HirOptionalValue {
                    optional,
                    source: crate::hir::HirOptionalValueSource::Copy(place),
                    span,
                });
            }
            if is_optional_producer(source) {
                let expression = self.check_expression(source)?;
                return Some(crate::hir::HirOptionalValue {
                    optional,
                    source: crate::hir::HirOptionalValueSource::Produced(Box::new(expression)),
                    span,
                });
            }
            self.diagnostics.push(
                Diagnostic::error(TYPE_MISMATCH, "nested optional source must be a value")
                    .with_primary_label(
                        span,
                        "this expression cannot produce stored optional data",
                    ),
            );
            return None;
        }

        if actual != payload {
            self.diagnostics.push(
                Diagnostic::error(
                    TYPE_MISMATCH,
                    format!(
                        "{context} requires `{}` or `{}`",
                        self.diagnostic_type_name(payload),
                        self.diagnostic_type_name(Type::Optional(optional))
                    ),
                )
                .with_primary_label(
                    span,
                    format!("source has type `{}`", self.diagnostic_type_name(actual)),
                )
                .with_note("implicit optional injection adds exactly one layer"),
            );
            return None;
        }

        let value = self.check_stored_value_initialization(payload, source, context)?;
        Some(crate::hir::HirOptionalValue {
            optional,
            source: crate::hir::HirOptionalValueSource::Present(Box::new(value)),
            span,
        })
    }

    pub(super) fn optional_value_place(
        &mut self,
        expression: &ResolvedExpression,
    ) -> Option<crate::hir::HirOptionalValuePlace> {
        match expression {
            ResolvedExpression::Binding(binding) => {
                let Type::Optional(optional) = self.binding_type(binding.binding) else {
                    return None;
                };
                Some(crate::hir::HirOptionalValuePlace {
                    storage: HirOptionalStorage::Binding(binding.binding),
                    optional,
                    span: binding.span,
                })
            }
            ResolvedExpression::Grouped(grouped) => self
                .optional_value_place(&grouped.expression)
                .map(|place| crate::hir::HirOptionalValuePlace {
                    span: grouped.span,
                    ..place
                }),
            ResolvedExpression::StaticFieldAccess(access) => {
                let (place, Type::Optional(optional)) =
                    self.check_static_place(access.field, access.span)?
                else {
                    return None;
                };
                Some(crate::hir::HirOptionalValuePlace {
                    storage: HirOptionalStorage::Static(place),
                    optional,
                    span: access.span,
                })
            }
            ResolvedExpression::FieldAccess(access) => {
                let expression = self.check_field_read(access)?;
                let Type::Optional(optional) = expression.ty else {
                    return None;
                };
                let HirExpressionKind::FieldRead(place) = expression.kind else {
                    unreachable!("field checking must produce a field-read expression");
                };
                Some(crate::hir::HirOptionalValuePlace {
                    storage: HirOptionalStorage::Field(place),
                    optional,
                    span: expression.span,
                })
            }
            ResolvedExpression::ArrayProjection(_) => {
                let expression = self.check_expression(expression)?;
                let Type::Optional(optional) = expression.ty else {
                    return None;
                };
                let HirExpressionKind::ArrayElement(place) = expression.kind else {
                    return None;
                };
                Some(crate::hir::HirOptionalValuePlace {
                    storage: HirOptionalStorage::ArrayElement(place),
                    optional,
                    span: expression.span,
                })
            }
            ResolvedExpression::Dereference(dereference) => {
                let pointee = self.check_optional_box_pointee(dereference)?;
                Some(crate::hir::HirOptionalValuePlace {
                    optional: pointee.optional,
                    span: pointee.span,
                    storage: HirOptionalStorage::SharedPointee(Box::new(pointee)),
                })
            }
            _ => None,
        }
    }

    pub(super) fn inline_optional_alias_place(
        &mut self,
        expression: &ResolvedExpression,
    ) -> Option<crate::hir::HirOptionalAliasPlace> {
        if matches!(expression, ResolvedExpression::ArrayProjection(_)) {
            return None;
        }
        self.optional_place(expression)
            .map(crate::hir::HirOptionalAliasPlace::Primitive)
            .or_else(|| {
                self.class_optional_place(expression)
                    .map(crate::hir::HirOptionalAliasPlace::Class)
            })
            .or_else(|| {
                self.optional_value_place(expression)
                    .map(crate::hir::HirOptionalAliasPlace::Nested)
            })
    }

    pub(super) fn check_optional_shared_initialize(
        &mut self,
        target: HirSharedTarget,
        source: &ResolvedExpression,
        context: &'static str,
    ) -> Option<HirOptionalSharedInitialize> {
        Some(HirOptionalSharedInitialize {
            target,
            source: self.check_optional_shared_source(source, target, context)?,
            span: source.span(),
        })
    }

    pub(super) fn check_optional_shared_assignment(
        &mut self,
        destination: HirOptionalSharedPlace,
        source: &ResolvedExpression,
        context: &'static str,
    ) -> Option<HirOptionalSharedAssignment> {
        Some(HirOptionalSharedAssignment {
            source: self.check_optional_shared_source(source, destination.target, context)?,
            destination,
            kind: crate::hir::HirOptionalWriteKind::Assign,
            span: source.span(),
        })
    }

    fn check_optional_shared_source(
        &mut self,
        source: &ResolvedExpression,
        target: HirSharedTarget,
        context: &'static str,
    ) -> Option<HirOptionalSharedSource> {
        if let ResolvedExpression::Absent(absent) = source {
            return Some(HirOptionalSharedSource::Absent { span: absent.span });
        }
        if let ResolvedExpression::Present(present) = source {
            return self
                .check_shared_transfer(&present.value, target, "`some` payload")
                .map(|transfer| HirOptionalSharedSource::Present(transfer.source));
        }
        if let Some(place) = self.optional_shared_place(source) {
            if super::shared::target_accepts(self.program, target, place.target) {
                return Some(HirOptionalSharedSource::Copy(place));
            }
            self.diagnostics.push(
                Diagnostic::error(
                    TYPE_MISMATCH,
                    format!(
                        "{context} requires `{}`",
                        self.optional_shared_target_name(target)
                    ),
                )
                .with_primary_label(
                    place.span,
                    format!(
                        "source has type `{}`",
                        self.optional_shared_target_name(place.target)
                    ),
                ),
            );
            return None;
        }
        if is_optional_producer(source) {
            let expression = self.check_expression(source)?;
            if let Some(OptionalPayloadKind::Shared(actual)) = self.optional_kind(expression.ty) {
                if super::shared::target_accepts(self.program, target, actual) {
                    return Some(HirOptionalSharedSource::Produced(Box::new(expression)));
                }
                self.diagnostics.push(
                    Diagnostic::error(
                        TYPE_MISMATCH,
                        format!(
                            "{context} requires `{}`",
                            self.optional_shared_target_name(target)
                        ),
                    )
                    .with_primary_label(
                        expression.span,
                        format!(
                            "source has type `{}`",
                            self.optional_shared_target_name(actual)
                        ),
                    ),
                );
                return None;
            }
        }
        self.check_shared_transfer(source, target, context)
            .map(|transfer| HirOptionalSharedSource::Present(transfer.source))
    }

    pub(super) fn optional_shared_place(
        &mut self,
        expression: &ResolvedExpression,
    ) -> Option<HirOptionalSharedPlace> {
        match expression {
            ResolvedExpression::Binding(binding) => {
                let Some(OptionalPayloadKind::Shared(target)) =
                    self.optional_kind(self.binding_type(binding.binding))
                else {
                    return None;
                };
                Some(HirOptionalSharedPlace {
                    storage: HirOptionalStorage::Binding(binding.binding),
                    target,
                    span: binding.span,
                })
            }
            ResolvedExpression::Grouped(grouped) => self
                .optional_shared_place(&grouped.expression)
                .map(|place| HirOptionalSharedPlace {
                    span: grouped.span,
                    ..place
                }),
            ResolvedExpression::StaticFieldAccess(access) => {
                let (place, ty) = self.check_static_place(access.field, access.span)?;
                let Some(OptionalPayloadKind::Shared(target)) = self.optional_kind(ty) else {
                    return None;
                };
                Some(HirOptionalSharedPlace {
                    storage: HirOptionalStorage::Static(place),
                    target,
                    span: access.span,
                })
            }
            ResolvedExpression::FieldAccess(access) => {
                let expression = self.check_field_read(access)?;
                let Some(OptionalPayloadKind::Shared(target)) = self.optional_kind(expression.ty)
                else {
                    return None;
                };
                let HirExpressionKind::FieldRead(place) = expression.kind else {
                    unreachable!("field checking must produce a field-read expression");
                };
                Some(HirOptionalSharedPlace {
                    storage: HirOptionalStorage::Field(place),
                    target,
                    span: expression.span,
                })
            }
            ResolvedExpression::ArrayProjection(_) => {
                let expression = self.check_expression(expression)?;
                let Some(OptionalPayloadKind::Shared(target)) = self.optional_kind(expression.ty)
                else {
                    return None;
                };
                let HirExpressionKind::ArrayElement(place) = expression.kind else {
                    return None;
                };
                Some(HirOptionalSharedPlace {
                    storage: HirOptionalStorage::ArrayElement(place),
                    target,
                    span: expression.span,
                })
            }
            ResolvedExpression::Dereference(dereference) => {
                let pointee = self.check_optional_box_pointee(dereference)?;
                let Some(OptionalPayloadKind::Shared(target)) =
                    self.optional_kind(Type::Optional(pointee.optional))
                else {
                    return None;
                };
                Some(HirOptionalSharedPlace {
                    storage: HirOptionalStorage::SharedPointee(Box::new(pointee)),
                    target,
                    span: dereference.span,
                })
            }
            _ => None,
        }
    }

    fn optional_shared_target_name(&self, target: HirSharedTarget) -> String {
        self.shared_target_name(target)
            .replacen("shared ", "shared? ", 1)
    }

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

    pub(in crate::typeck) fn check_class_optional_source(
        &mut self,
        source: &ResolvedExpression,
        class: crate::identity::ClassId,
        context: &'static str,
    ) -> Option<HirClassOptionalSource> {
        if let ResolvedExpression::Absent(absent) = source {
            return Some(HirClassOptionalSource::Absent { span: absent.span });
        }
        if let ResolvedExpression::Present(present) = source {
            return self
                .check_object_source(&present.value, class, "`some` payload")
                .map(HirClassOptionalSource::Present);
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
        if is_optional_producer(source) {
            let expression = self.check_expression(source)?;
            if let Some(OptionalPayloadKind::Class(actual)) = self.optional_kind(expression.ty) {
                if actual == class {
                    return Some(HirClassOptionalSource::Produced(Box::new(expression)));
                }
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
            if is_call_through_groups(source) && expression.ty == Type::Class(class) {
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
                    format!(
                        "source has type `{}`",
                        self.diagnostic_type_name(expression.ty)
                    ),
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
                let Some(OptionalPayloadKind::Class(class)) =
                    self.optional_kind(self.binding_type(binding.binding))
                else {
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
            ResolvedExpression::StaticFieldAccess(access) => {
                let (place, ty) = self.check_static_place(access.field, access.span)?;
                let Some(OptionalPayloadKind::Class(class)) = self.optional_kind(ty) else {
                    return None;
                };
                Some(HirClassOptionalPlace {
                    storage: HirOptionalStorage::Static(place),
                    class,
                    span: access.span,
                })
            }
            ResolvedExpression::FieldAccess(access) => {
                let expression = self.check_field_read(access)?;
                let Some(OptionalPayloadKind::Class(class)) = self.optional_kind(expression.ty)
                else {
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
            ResolvedExpression::ArrayProjection(_) => {
                let expression = self.check_expression(expression)?;
                let Some(OptionalPayloadKind::Class(class)) = self.optional_kind(expression.ty)
                else {
                    return None;
                };
                let HirExpressionKind::ArrayElement(place) = expression.kind else {
                    return None;
                };
                Some(HirClassOptionalPlace {
                    storage: HirOptionalStorage::ArrayElement(place),
                    class,
                    span: expression.span,
                })
            }
            ResolvedExpression::Dereference(dereference) => {
                let pointee = self.check_optional_box_pointee(dereference)?;
                let Some(OptionalPayloadKind::Class(class)) =
                    self.optional_kind(Type::Optional(pointee.optional))
                else {
                    return None;
                };
                Some(HirClassOptionalPlace {
                    storage: HirOptionalStorage::SharedPointee(Box::new(pointee)),
                    class,
                    span: dereference.span,
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
        if let ResolvedExpression::Present(present) = source {
            let value = self.check_expression(&present.value)?;
            if value.ty != payload.value_type() {
                self.diagnostics.push(
                    Diagnostic::error(
                        TYPE_MISMATCH,
                        format!("`some` payload requires `{}`", payload.name()),
                    )
                    .with_primary_label(
                        value.span,
                        format!("source has type `{}`", self.diagnostic_type_name(value.ty)),
                    ),
                );
                return None;
            }
            return Some(HirOptionalSource::Present(value));
        }
        if let Some(place) = self.optional_place(source) {
            if place.payload == payload {
                return Some(HirOptionalSource::Copy(place));
            }
            self.report_optional_payload_mismatch(place.payload, payload, place.span, context);
            return None;
        }

        let value = self.check_expression(source)?;
        if self.optional_kind(value.ty) == Some(OptionalPayloadKind::Primitive(payload)) {
            return Some(HirOptionalSource::Produced(Box::new(value)));
        }
        if value.ty != payload.value_type() {
            self.diagnostics.push(
                Diagnostic::error(
                    TYPE_MISMATCH,
                    format!(
                        "{context} requires `{}` or `{}?`",
                        payload.name(),
                        payload.name()
                    ),
                )
                .with_primary_label(
                    value.span,
                    format!("source has type `{}`", self.diagnostic_type_name(value.ty)),
                ),
            );
            return None;
        }
        Some(HirOptionalSource::Present(value))
    }

    pub(super) fn check_presence_test(
        &mut self,
        test: &ResolvedPresenceTestExpr,
    ) -> Option<HirExpression> {
        let kind = match test.kind {
            ResolvedPresenceTestKind::Some => HirPresenceTestKind::Some,
            ResolvedPresenceTestKind::None => HirPresenceTestKind::None,
        };
        if let Some((source, box_target)) = self.check_optional_box_presence_source(&test.source) {
            return Some(HirExpression {
                kind: HirExpressionKind::OptionalBoxPresence(crate::hir::HirOptionalBoxPresence {
                    source,
                    box_target,
                    kind,
                    span: test.span,
                }),
                ty: Type::Bool,
                span: test.span,
            });
        }
        let source = self.require_optional_operand(&test.source, test.span, "presence test")?;
        Some(HirExpression {
            kind: HirExpressionKind::PresenceTest { source, kind },
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
        let aggregate_optional = match &source {
            HirOptionalOperand::AggregatePlace(place) => Some(place.optional),
            HirOptionalOperand::AggregateProduced(expression) => {
                let Type::Optional(optional) = expression.ty else {
                    unreachable!("aggregate optional producer must retain its optional type")
                };
                Some(optional)
            }
            _ => None,
        };
        if let Some(optional) = aggregate_optional {
            if let Type::Array(array) = super::optional_types::payload_type(self.program, optional)
            {
                return Some(HirExpression {
                    kind: HirExpressionKind::OptionalArrayUnwrap(Box::new(
                        crate::hir::HirOptionalArrayUnwrap {
                            source,
                            optional,
                            array,
                            span: unwrap.span,
                        },
                    )),
                    ty: Type::Array(array),
                    span: unwrap.span,
                });
            }
        }
        if let HirOptionalOperand::AggregatePlace(place) = &source {
            let optional = place.optional;
            let Type::Optional(payload) =
                super::optional_types::payload_type(self.program, optional)
            else {
                unreachable!("nested optional metadata must have an optional payload")
            };
            return Some(HirExpression {
                kind: HirExpressionKind::NestedOptionalUnwrap(Box::new(
                    crate::hir::HirNestedOptionalUnwrap {
                        source,
                        optional,
                        payload,
                        span: unwrap.span,
                    },
                )),
                ty: Type::Optional(payload),
                span: unwrap.span,
            });
        }
        if let HirOptionalOperand::AggregateProduced(expression) = &source {
            let Type::Optional(outer) = expression.ty else {
                unreachable!("nested optional producer must retain its optional type")
            };
            let Type::Optional(payload) = super::optional_types::payload_type(self.program, outer)
            else {
                unreachable!("nested optional metadata must have an optional payload")
            };
            return Some(HirExpression {
                kind: HirExpressionKind::NestedOptionalUnwrap(Box::new(
                    crate::hir::HirNestedOptionalUnwrap {
                        source,
                        optional: outer,
                        payload,
                        span: unwrap.span,
                    },
                )),
                ty: Type::Optional(payload),
                span: unwrap.span,
            });
        }
        if matches!(
            source,
            HirOptionalOperand::ClassPlace(_)
                | HirOptionalOperand::ClassProduced(_)
                | HirOptionalOperand::SharedPlace(_)
                | HirOptionalOperand::SharedProduced(_)
                | HirOptionalOperand::AggregatePlace(_)
                | HirOptionalOperand::AggregateProduced(_)
        ) {
            self.diagnostics.push(
                Diagnostic::error(
                    crate::typeck::program::INVALID_OBJECT_CONTEXT,
                    "this optional payload is not a scalar value",
                )
                .with_primary_label(
                    unwrap.span,
                    "consume this checked payload as a member receiver, alias, cast, type test, or copy source",
                ),
            );
            return None;
        }
        let payload = match &source {
            HirOptionalOperand::Place(place) => place.payload,
            HirOptionalOperand::Produced(expression) => {
                let Some(OptionalPayloadKind::Primitive(payload)) =
                    self.optional_kind(expression.ty)
                else {
                    unreachable!("primitive optional operand must retain primitive metadata")
                };
                payload
            }
            _ => unreachable!("non-scalar optional operands were rejected above"),
        };
        Some(HirExpression {
            kind: HirExpressionKind::Unwrap(source),
            ty: payload.value_type(),
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
                HirOptionalStorage::Static(_) => crate::hir::HirAccess::Mutable,
                HirOptionalStorage::Field(field) => field.receiver.access,
                HirOptionalStorage::ArrayElement(place) => place.receiver.access,
                HirOptionalStorage::SharedPointee(_) => crate::hir::HirAccess::Mutable,
            },
            HirOptionalOperand::ClassProduced(_) => crate::hir::HirAccess::Mutable,
            HirOptionalOperand::Place(_)
            | HirOptionalOperand::Produced(_)
            | HirOptionalOperand::SharedPlace(_)
            | HirOptionalOperand::SharedProduced(_)
            | HirOptionalOperand::AggregatePlace(_)
            | HirOptionalOperand::AggregateProduced(_) => {
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

    pub(super) fn require_optional_operand(
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
        if let Some(place) = self.optional_shared_place(expression) {
            return Some(HirOptionalOperand::SharedPlace(place));
        }
        if matches!(
            self.optional_kind(self.static_expression_type(expression)),
            Some(OptionalPayloadKind::Nested(_) | OptionalPayloadKind::Array(_))
        ) {
            if let Some(place) = self.optional_value_place(expression) {
                return Some(HirOptionalOperand::AggregatePlace(place));
            }
        }
        if is_optional_producer(expression) {
            if let Some(value) = self.check_expression(expression) {
                if let Some(kind) = self.optional_kind(value.ty) {
                    return Some(match kind {
                        OptionalPayloadKind::Primitive(_) => {
                            HirOptionalOperand::Produced(Box::new(value))
                        }
                        OptionalPayloadKind::Class(_) => {
                            HirOptionalOperand::ClassProduced(Box::new(value))
                        }
                        OptionalPayloadKind::Shared(_) => {
                            HirOptionalOperand::SharedProduced(Box::new(value))
                        }
                        OptionalPayloadKind::Nested(_) => {
                            HirOptionalOperand::AggregateProduced(Box::new(value))
                        }
                        OptionalPayloadKind::Array(_) => {
                            HirOptionalOperand::AggregateProduced(Box::new(value))
                        }
                    });
                }
                self.report_non_optional_operand(value.ty, span, context);
                return None;
            }
        }
        let actual = self.check_expression(expression).map(|value| value.ty);
        let label = actual.map_or_else(
            || "expected a primitive optional local".to_owned(),
            |ty| {
                format!(
                    "expression has non-optional type `{}`",
                    self.diagnostic_type_name(ty)
                )
            },
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
                let Some(OptionalPayloadKind::Primitive(payload)) =
                    self.optional_kind(self.binding_type(binding.binding))
                else {
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
            ResolvedExpression::StaticFieldAccess(access) => {
                let (place, ty) = self.check_static_place(access.field, access.span)?;
                let Some(OptionalPayloadKind::Primitive(payload)) = self.optional_kind(ty) else {
                    return None;
                };
                Some(HirOptionalPlace {
                    storage: HirOptionalStorage::Static(place),
                    payload,
                    span: access.span,
                })
            }
            ResolvedExpression::FieldAccess(access) => {
                let expression = self.check_field_read(access)?;
                let Some(OptionalPayloadKind::Primitive(payload)) =
                    self.optional_kind(expression.ty)
                else {
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
            ResolvedExpression::ArrayProjection(_) => {
                let expression = self.check_expression(expression)?;
                let Some(OptionalPayloadKind::Primitive(payload)) =
                    self.optional_kind(expression.ty)
                else {
                    return None;
                };
                let HirExpressionKind::ArrayElement(place) = expression.kind else {
                    return None;
                };
                Some(HirOptionalPlace {
                    storage: HirOptionalStorage::ArrayElement(place),
                    payload,
                    span: expression.span,
                })
            }
            ResolvedExpression::Dereference(dereference) => {
                let pointee = self.check_optional_box_pointee(dereference)?;
                let Some(OptionalPayloadKind::Primitive(payload)) =
                    self.optional_kind(Type::Optional(pointee.optional))
                else {
                    return None;
                };
                Some(HirOptionalPlace {
                    storage: HirOptionalStorage::SharedPointee(Box::new(pointee)),
                    payload,
                    span: dereference.span,
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
                format!(
                    "expression has non-optional type `{}`",
                    self.diagnostic_type_name(actual)
                ),
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

fn is_optional_producer(expression: &ResolvedExpression) -> bool {
    match expression {
        ResolvedExpression::DirectCall(_)
        | ResolvedExpression::StaticCall(_)
        | ResolvedExpression::MethodCall(_)
        | ResolvedExpression::InterfaceCall(_)
        | ResolvedExpression::Unwrap(_) => true,
        ResolvedExpression::Grouped(grouped) => is_optional_producer(&grouped.expression),
        _ => false,
    }
}
