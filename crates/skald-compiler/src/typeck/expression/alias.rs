//! Non-owning alias sources, access checks, and static view conversions.

use super::object_view::{
    ObjectViewSource, ObjectViewSourceAdmission, ObjectViewSourceDiagnosticContext,
};
use super::*;

use crate::{
    hir::{
        HirAccess, HirCallArgument, HirObjectOrigin, HirObjectPlace, HirObjectView, HirViewSource,
        HirViewTarget, Type,
    },
    identity::BindingId,
    resolve::ResolvedExpression,
    source::Span,
    typeck::program::{
        lower_parameter_mode, lower_type, INSUFFICIENT_ALIAS_ACCESS, INVALID_ALIAS_ARGUMENT,
    },
};

const ALIAS_OBJECT_VIEW_SOURCE: ObjectViewSourceDiagnosticContext = ObjectViewSourceDiagnosticContext::new(
    "alias argument source",
    INVALID_ALIAS_ARGUMENT,
    "alias argument must designate an object",
    "alias argument must use an object place, an explicit shared dereference, or a compatible produced object",
);

const ITERATION_OBJECT_VIEW_SOURCE: ObjectViewSourceDiagnosticContext =
    ObjectViewSourceDiagnosticContext::new(
        "iteration receiver",
        crate::typeck::program::GENERAL_ITERATION_UNSUPPORTED,
        "iteration requires a read-only object receiver",
        "iteration requires an object view that is safe for the whole loop",
    );

impl CallableChecker<'_, '_> {
    pub(super) fn check_alias_argument(
        &mut self,
        expression: &ResolvedExpression,
        parameter: &impl CallParameter,
    ) -> Option<HirCallArgument> {
        let expected = lower_type(self.program, parameter.type_syntax());
        if matches!(expected, Type::Array(_)) || is_array_projection_through_groups(expression) {
            return self.check_array_alias_argument(expression, parameter);
        }
        if matches!(
            expected,
            Type::I64 | Type::U64 | Type::U8 | Type::F64 | Type::Bool
        ) {
            return self.check_primitive_alias_argument(expression, expected, parameter);
        }
        if matches!(expected, Type::Shared(_)) {
            return self.check_shared_owner_alias_argument(expression, expected, parameter);
        }
        if matches!(expected, Type::Optional(_))
            && matches!(
                self.optional_kind(expected),
                Some(super::super::optional_types::OptionalPayloadKind::Shared(_))
            )
        {
            return self
                .check_optional_shared_owner_alias_argument(expression, expected, parameter);
        }
        if matches!(expected, Type::Optional(_))
            && !matches!(
                self.optional_kind(expected),
                Some(super::super::optional_types::OptionalPayloadKind::Shared(_))
            )
        {
            return self.check_optional_alias_argument(expression, expected, parameter);
        }
        if let Some(target) = self.resolved_shared_target(expression) {
            let diagnostic = self
                .implicit_shared_dereference_diagnostic(expression.span(), target)
                .with_secondary_label(parameter.span(), "alias parameter declared here")
                .with_note(ALIAS_OBJECT_VIEW_SOURCE.place_message());
            self.diagnostics.push(diagnostic);
            return None;
        }
        let required = lower_parameter_mode(parameter.binding_mode())
            .required_access()
            .expect("alias parameter mode must require place access");
        if required == HirAccess::Mutable && self.is_produced_alias_source(expression) {
            self.diagnostics.push(
                Diagnostic::error(
                    INVALID_ALIAS_ARGUMENT,
                    "mutable alias argument requires an existing object place",
                )
                .with_primary_label(
                    expression.span(),
                    "this expression produces a temporary object",
                )
                .with_secondary_label(parameter.span(), "mutable alias declared here"),
            );
            return None;
        }
        if let ResolvedExpression::ObjectCast(cast) = expression {
            return self.check_cast_alias_argument(cast, parameter);
        }
        if let ResolvedExpression::Grouped(grouped) = expression {
            if matches!(*grouped.expression, ResolvedExpression::ObjectCast(_)) {
                return self.check_alias_argument(&grouped.expression, parameter);
            }
        }
        let source = self.check_object_view_source(
            expression,
            ObjectViewSourceAdmission::ExistingOrProducedObject,
            ALIAS_OBJECT_VIEW_SOURCE,
        )?;
        if !source.access().permits(required) {
            self.diagnostics.push(
                Diagnostic::error(
                    INSUFFICIENT_ALIAS_ACCESS,
                    "read-only access cannot satisfy a mutable alias parameter",
                )
                .with_primary_label(source.span(), "this place provides read-only access")
                .with_secondary_label(parameter.span(), "mutable alias declared here"),
            );
            return None;
        }
        self.convert_alias_argument(
            source,
            lower_type(self.program, parameter.type_syntax()),
            required,
            parameter,
        )
    }

    fn check_shared_owner_alias_argument(
        &mut self,
        expression: &ResolvedExpression,
        expected: Type,
        parameter: &impl CallParameter,
    ) -> Option<HirCallArgument> {
        let source = self.check_shared_source(expression, false)?;
        let crate::hir::HirSharedSource::Place(place) = source else {
            self.report_non_place_alias(expression, parameter, "shared-owner");
            return None;
        };
        let actual = Type::Shared(place.target());
        if actual != expected {
            self.report_exact_alias_type_mismatch(actual, expected, place.span(), parameter);
            return None;
        }
        let required = lower_parameter_mode(parameter.binding_mode())
            .required_access()
            .expect("alias parameter mode must require place access");
        let access = self.shared_place_access(&place)?;
        if !access.permits(required) {
            self.report_alias_access_failure(place.span(), parameter, "shared-owner");
            return None;
        }
        Some(HirCallArgument::SharedPlace(place))
    }

    fn check_optional_shared_owner_alias_argument(
        &mut self,
        expression: &ResolvedExpression,
        expected: Type,
        parameter: &impl CallParameter,
    ) -> Option<HirCallArgument> {
        let Some(place) = self.optional_shared_place(expression) else {
            self.report_non_place_alias(expression, parameter, "optional shared-owner");
            return None;
        };
        let actual = self.static_expression_type(expression);
        if actual != expected {
            self.report_exact_alias_type_mismatch(actual, expected, place.span, parameter);
            return None;
        }
        let required = lower_parameter_mode(parameter.binding_mode())
            .required_access()
            .expect("alias parameter mode must require place access");
        let access = self.optional_storage_access(&place.storage, place.span)?;
        if !access.permits(required) {
            self.report_alias_access_failure(place.span, parameter, "optional shared-owner");
            return None;
        }
        Some(HirCallArgument::OptionalSharedPlace(place))
    }

    fn shared_place_access(&mut self, place: &crate::hir::HirSharedPlace) -> Option<HirAccess> {
        match place {
            crate::hir::HirSharedPlace::Binding { binding, span, .. } => {
                self.binding_access(*binding, false, *span)
            }
            crate::hir::HirSharedPlace::Field { place, .. } => {
                Some(self.rebinding_field_place_alias_access(place))
            }
            crate::hir::HirSharedPlace::ArrayElement { place, .. } => Some(place.receiver.access),
            crate::hir::HirSharedPlace::Static { place, .. } => {
                Some(self.rebinding_static_field_alias_access(place.field))
            }
        }
    }

    fn report_non_place_alias(
        &mut self,
        expression: &ResolvedExpression,
        parameter: &impl CallParameter,
        kind: &str,
    ) {
        self.diagnostics.push(
            Diagnostic::error(
                INVALID_ALIAS_ARGUMENT,
                format!("{kind} alias argument must designate existing storage"),
            )
            .with_primary_label(
                expression.span(),
                "this expression produces a temporary owner",
            )
            .with_secondary_label(parameter.span(), "alias parameter declared here"),
        );
    }

    fn report_exact_alias_type_mismatch(
        &mut self,
        actual: Type,
        expected: Type,
        span: Span,
        parameter: &impl CallParameter,
    ) {
        self.diagnostics.push(
            Diagnostic::error(
                TYPE_MISMATCH,
                format!(
                    "alias argument has type `{}`, but `{}` is required",
                    self.diagnostic_type_name(actual),
                    self.diagnostic_type_name(expected),
                ),
            )
            .with_primary_label(span, "this owner place has a different exact type")
            .with_secondary_label(
                parameter.type_syntax().span,
                "alias parameter type declared here",
            ),
        );
    }

    fn report_alias_access_failure(
        &mut self,
        span: Span,
        parameter: &impl CallParameter,
        kind: &str,
    ) {
        self.diagnostics.push(
            Diagnostic::error(
                INSUFFICIENT_ALIAS_ACCESS,
                format!("read-only {kind} access cannot satisfy a mutable alias parameter"),
            )
            .with_primary_label(span, "this owner place provides read-only access")
            .with_secondary_label(parameter.span(), "mutable alias declared here"),
        );
    }

    fn check_primitive_alias_argument(
        &mut self,
        expression: &ResolvedExpression,
        expected: Type,
        parameter: &impl CallParameter,
    ) -> Option<HirCallArgument> {
        let required = lower_parameter_mode(parameter.binding_mode())
            .required_access()
            .expect("alias parameter mode must require place access");
        if let Some((place, actual, access)) = self.primitive_alias_place(expression) {
            if actual != expected {
                self.report_primitive_alias_type_mismatch(place.span, actual, expected, parameter);
                return None;
            }
            if !access.permits(required) {
                self.diagnostics.push(
                    Diagnostic::error(
                        INSUFFICIENT_ALIAS_ACCESS,
                        "read-only primitive access cannot satisfy a mutable alias parameter",
                    )
                    .with_primary_label(place.span, "this place provides read-only access")
                    .with_secondary_label(parameter.span(), "mutable alias declared here"),
                );
                return None;
            }
            return Some(HirCallArgument::PrimitivePlace(place));
        }

        let produced = self.check_expression(expression)?;
        if required == HirAccess::Mutable {
            self.diagnostics.push(
                Diagnostic::error(
                    INVALID_ALIAS_ARGUMENT,
                    "mutable primitive alias argument requires an existing primitive place",
                )
                .with_primary_label(
                    expression.span(),
                    "this expression produces a primitive value",
                )
                .with_secondary_label(parameter.span(), "mutable alias declared here"),
            );
            return None;
        }
        if produced.ty != expected {
            self.report_primitive_alias_type_mismatch(
                produced.span,
                produced.ty,
                expected,
                parameter,
            );
            return None;
        }
        Some(HirCallArgument::ProducedPrimitiveAlias(produced))
    }

    fn report_primitive_alias_type_mismatch(
        &mut self,
        span: Span,
        actual: Type,
        expected: Type,
        parameter: &impl CallParameter,
    ) {
        self.diagnostics.push(
            Diagnostic::error(
                TYPE_MISMATCH,
                format!(
                    "primitive alias argument has type `{}` but `{}` is required",
                    actual.name(),
                    expected.name()
                ),
            )
            .with_primary_label(span, "this expression has a different primitive type")
            .with_secondary_label(
                parameter.type_syntax().span,
                "alias parameter type declared here",
            ),
        );
    }

    fn primitive_alias_place(
        &mut self,
        expression: &ResolvedExpression,
    ) -> Option<(crate::hir::HirPrimitivePlace, Type, HirAccess)> {
        match expression {
            ResolvedExpression::Binding(binding) => {
                let ty = self.binding_type(binding.binding);
                matches!(
                    ty,
                    Type::I64 | Type::U64 | Type::U8 | Type::F64 | Type::Bool
                )
                .then(|| {
                    let access = self.binding_access(binding.binding, false, binding.span)?;
                    Some((
                        crate::hir::HirPrimitivePlace {
                            storage: crate::hir::HirPrimitiveStorage::Binding(binding.binding),
                            span: binding.span,
                        },
                        ty,
                        access,
                    ))
                })
                .flatten()
            }
            ResolvedExpression::StaticFieldAccess(access) => self
                .primitive_static_alias_place(access)
                .map(|(place, ty)| {
                    (
                        place,
                        ty,
                        self.rebinding_static_field_alias_access(access.field),
                    )
                }),
            ResolvedExpression::Grouped(grouped) => {
                let (mut place, ty, access) = self.primitive_alias_place(&grouped.expression)?;
                place.span = grouped.span;
                Some((place, ty, access))
            }
            _ => None,
        }
    }

    fn check_optional_alias_argument(
        &mut self,
        expression: &ResolvedExpression,
        expected: Type,
        parameter: &impl CallParameter,
    ) -> Option<HirCallArgument> {
        if let ResolvedExpression::Dereference(dereference) = expression {
            if matches!(
                dereference.target,
                crate::resolve::ResolvedSharedTarget::OptionalBox(_)
            ) {
                let mutable = matches!(
                    parameter.binding_mode(),
                    crate::resolve::ResolvedParameterBindingMode::MutableAlias { .. }
                );
                if mutable {
                    self.diagnostics.push(
                        Diagnostic::error(
                            INVALID_ALIAS_ARGUMENT,
                            "a published optional-box wrapper cannot be mutably aliased",
                        )
                        .with_primary_label(
                            dereference.operator_span,
                            "the complete boxed optional is immutable after publication",
                        )
                        .with_secondary_label(parameter.span(), "optional alias declared here"),
                    );
                    return None;
                }
            }
        }
        let place = self.inline_optional_alias_place(expression);
        let Some(place) = place else {
            self.diagnostics.push(
                Diagnostic::error(
                    INVALID_ALIAS_ARGUMENT,
                    "optional alias argument must designate an existing optional container",
                )
                .with_primary_label(
                    expression.span(),
                    "pass an optional local, parameter, field, or grouping",
                )
                .with_secondary_label(parameter.span(), "optional alias declared here"),
            );
            return None;
        };
        let actual = self.static_expression_type(expression);
        if actual != expected {
            self.diagnostics.push(
                Diagnostic::error(
                    TYPE_MISMATCH,
                    format!(
                        "optional alias argument has type `{}`, but `{}` is required",
                        self.diagnostic_type_name(actual),
                        self.diagnostic_type_name(expected)
                    ),
                )
                .with_primary_label(place.span(), "this optional container has a different type")
                .with_secondary_label(
                    parameter.type_syntax().span,
                    "alias parameter type declared here",
                ),
            );
            return None;
        }
        let required = lower_parameter_mode(parameter.binding_mode())
            .required_access()
            .expect("alias parameter mode must require place access");
        let access = match &place {
            crate::hir::HirOptionalAliasPlace::Primitive(place) => {
                self.optional_storage_access(&place.storage, place.span)?
            }
            crate::hir::HirOptionalAliasPlace::Class(place) => {
                self.optional_storage_access(&place.storage, place.span)?
            }
            crate::hir::HirOptionalAliasPlace::Nested(place) => {
                self.optional_storage_access(&place.storage, place.span)?
            }
        };
        if !access.permits(required) {
            self.diagnostics.push(
                Diagnostic::error(
                    INSUFFICIENT_ALIAS_ACCESS,
                    "read-only optional access cannot satisfy a mutable alias parameter",
                )
                .with_primary_label(place.span(), "this container provides read-only access")
                .with_secondary_label(parameter.span(), "mutable alias declared here"),
            );
            return None;
        }
        Some(HirCallArgument::OptionalPlace(place))
    }

    fn optional_storage_access(
        &mut self,
        storage: &crate::hir::HirOptionalStorage,
        span: Span,
    ) -> Option<HirAccess> {
        match storage {
            crate::hir::HirOptionalStorage::Binding(binding) => {
                self.binding_access(*binding, false, span)
            }
            crate::hir::HirOptionalStorage::Static(place) => {
                Some(self.rebinding_static_field_alias_access(place.field))
            }
            crate::hir::HirOptionalStorage::Field(field) => {
                Some(self.rebinding_field_place_alias_access(field))
            }
            crate::hir::HirOptionalStorage::ArrayElement(place) => Some(place.receiver.access),
            crate::hir::HirOptionalStorage::SharedPointee(_) => Some(HirAccess::ReadOnly),
        }
    }

    fn check_cast_alias_argument(
        &mut self,
        cast: &crate::resolve::ResolvedObjectCastExpr,
        parameter: &impl CallParameter,
    ) -> Option<HirCallArgument> {
        let mut checked = self.check_object_cast(cast)?;
        let required = lower_parameter_mode(parameter.binding_mode())
            .required_access()
            .expect("alias parameter mode must require place access");
        if !checked.view.access.permits(required) {
            self.diagnostics.push(
                Diagnostic::error(
                    INSUFFICIENT_ALIAS_ACCESS,
                    "read-only cast place cannot satisfy a mutable alias parameter",
                )
                .with_primary_label(cast.span, "this cast preserves read-only source access")
                .with_secondary_label(parameter.span(), "mutable alias declared here"),
            );
            return None;
        }
        let expected = lower_type(self.program, parameter.type_syntax());
        let expected_target = match expected {
            Type::Class(class) => HirViewTarget::Class(class),
            Type::Interface(interface) => HirViewTarget::Interface(interface),
            Type::Obj => HirViewTarget::Obj,
            primitive => {
                self.diagnostics.push(
                    Diagnostic::error(
                        TYPE_MISMATCH,
                        format!(
                            "cast place cannot satisfy value parameter type `{}`",
                            primitive.name()
                        ),
                    )
                    .with_primary_label(cast.span, "this is a non-owning object place"),
                );
                return None;
            }
        };
        let cast_target = checked.view.target;
        let compatible = match (cast_target, expected_target) {
            (actual, expected) if actual == expected => true,
            (HirViewTarget::Class(actual), HirViewTarget::Class(expected)) => {
                let mut current = actual;
                while current != expected {
                    let Some(base) = self.program.hierarchy.direct_base(current) else {
                        break;
                    };
                    checked
                        .projections
                        .push(crate::object_path::ObjectProjection::Base(base));
                    current = base;
                }
                if current == expected {
                    checked.class = Some(expected);
                    true
                } else {
                    false
                }
            }
            (HirViewTarget::Class(_), HirViewTarget::Obj) => true,
            (HirViewTarget::Class(actual), HirViewTarget::Interface(interface)) => {
                class_provides_view(self.program, actual, HirViewTarget::Interface(interface))
            }
            _ => false,
        };
        if !compatible {
            self.diagnostics.push(
                Diagnostic::error(
                    TYPE_MISMATCH,
                    format!("cast place is incompatible with `{}`", expected.name()),
                )
                .with_primary_label(cast.span, "this cast cannot be implicitly converted")
                .with_secondary_label(
                    parameter.type_syntax().span,
                    "alias parameter type declared here",
                ),
            );
            return None;
        }
        checked.consumer_target = expected_target;
        checked.consumer_access = required;
        Some(HirCallArgument::CheckedView(Box::new(checked)))
    }

    fn is_produced_alias_source(&self, expression: &ResolvedExpression) -> bool {
        match expression {
            ResolvedExpression::Construct(_)
            | ResolvedExpression::StringLiteral(_)
            | ResolvedExpression::DirectCall(_)
            | ResolvedExpression::IndirectCall(_)
            | ResolvedExpression::StaticCall(_)
            | ResolvedExpression::MethodCall(_)
            | ResolvedExpression::InterfaceCall(_) => {
                self.resolved_object_class(expression).is_some()
            }
            ResolvedExpression::Grouped(grouped) => {
                self.is_produced_alias_source(&grouped.expression)
            }
            ResolvedExpression::ObjectCast(cast) => self.is_produced_alias_source(&cast.source),
            ResolvedExpression::FieldAccess(access) => matches!(
                access.receiver,
                crate::resolve::ResolvedObjectReceiver::Produced { .. }
            ),
            _ => false,
        }
    }

    /// Builds a read-only loop-duration view using the ordinary object-view
    /// source rules. Shared bindings are deliberately anchored: the loop body
    /// may replace the owner binding without invalidating the retained view.
    pub(in crate::typeck) fn check_iteration_view(
        &mut self,
        expression: &ResolvedExpression,
        target: HirViewTarget,
    ) -> Option<(Type, HirObjectView)> {
        let source = self.check_object_view_source(
            expression,
            ObjectViewSourceAdmission::ExistingOrProducedObject,
            ITERATION_OBJECT_VIEW_SOURCE,
        )?;
        let iterable = view_target_type(source.static_target());
        debug_assert!(source.access().permits(HirAccess::ReadOnly));
        let source = match source {
            ObjectViewSource::Shared(shared) => {
                ObjectViewSource::Shared(shared.into_iteration_source())
            }
            source => source,
        };
        Some((iterable, source.into_view(target, HirAccess::ReadOnly)))
    }

    fn convert_alias_argument(
        &mut self,
        source: ObjectViewSource,
        expected: Type,
        required: HirAccess,
        parameter: &impl CallParameter,
    ) -> Option<HirCallArgument> {
        let source_span = source.span();
        let mismatch = |actual: &str, expected: &str, span, label| {
            Diagnostic::error(
                TYPE_MISMATCH,
                format!("alias argument has type `{actual}`, expected `{expected}`"),
            )
            .with_primary_label(span, label)
            .with_secondary_label(
                parameter.type_syntax().span,
                "alias parameter type declared here",
            )
        };

        match (source, expected) {
            (ObjectViewSource::Class { place, origin }, Type::Class(target)) => {
                let actual = place.class();
                let Some(projected) = self.project_place_to_ancestor(place, target) else {
                    let actual_name = self
                        .program
                        .class(actual)
                        .expect("alias source class must exist")
                        .name
                        .clone();
                    let expected_name = self
                        .program
                        .class(target)
                        .expect("alias target class must exist")
                        .name
                        .clone();
                    self.diagnostics.push(mismatch(
                        &actual_name,
                        &expected_name,
                        source_span,
                        "this place has the wrong class",
                    ));
                    return None;
                };
                let span = projected.span();
                Some(HirCallArgument::View(HirObjectView {
                    source: HirViewSource::Place(projected),
                    origin: Box::new(origin),
                    target: HirViewTarget::Class(target),
                    access: required,
                    span,
                }))
            }
            (ObjectViewSource::Class { place, origin }, Type::Obj) => {
                let span = place.span();
                Some(HirCallArgument::View(HirObjectView {
                    source: HirViewSource::Place(place),
                    origin: Box::new(origin),
                    target: HirViewTarget::Obj,
                    access: required,
                    span,
                }))
            }
            (ObjectViewSource::Class { place, origin }, Type::Interface(interface)) => {
                let actual = place.class();
                if !class_provides_view(self.program, actual, HirViewTarget::Interface(interface)) {
                    let interface_name = &self
                        .program
                        .interface(interface)
                        .expect("alias target interface must exist")
                        .name;
                    self.diagnostics.push(mismatch(
                        &self
                            .program
                            .class(actual)
                            .expect("source class must exist")
                            .name,
                        interface_name,
                        source_span,
                        "this class does not implement the target interface",
                    ));
                    return None;
                }
                let span = place.span();
                Some(HirCallArgument::View(HirObjectView {
                    source: HirViewSource::Place(place),
                    origin: Box::new(origin),
                    target: HirViewTarget::Interface(interface),
                    access: required,
                    span,
                }))
            }
            (
                ObjectViewSource::Obj {
                    binding,
                    access,
                    span,
                },
                Type::Obj,
            ) => Some(forwarded_view(
                binding,
                HirViewTarget::Obj,
                HirViewTarget::Obj,
                access,
                required,
                span,
            )),
            (ObjectViewSource::Obj { span, .. }, Type::Class(target)) => {
                let target_name = self
                    .program
                    .class(target)
                    .expect("alias target class must exist")
                    .name
                    .clone();
                self.diagnostics.push(mismatch(
                    "Obj",
                    &target_name,
                    span,
                    "an `Obj` view cannot convert implicitly to a class",
                ));
                None
            }
            (
                ObjectViewSource::Interface {
                    binding,
                    interface,
                    access,
                    span,
                },
                Type::Interface(target),
            ) if interface == target => Some(forwarded_view(
                binding,
                HirViewTarget::Interface(interface),
                HirViewTarget::Interface(target),
                access,
                required,
                span,
            )),
            (
                ObjectViewSource::Interface {
                    binding,
                    interface,
                    access,
                    span,
                },
                Type::Obj,
            ) => Some(forwarded_view(
                binding,
                HirViewTarget::Interface(interface),
                HirViewTarget::Obj,
                access,
                required,
                span,
            )),
            (ObjectViewSource::Interface { span, .. }, Type::Class(target)) => {
                let target_name = &self
                    .program
                    .class(target)
                    .expect("target class must exist")
                    .name;
                self.diagnostics.push(mismatch(
                    "interface view",
                    target_name,
                    span,
                    "an interface view cannot convert implicitly to a class",
                ));
                None
            }
            (
                ObjectViewSource::Interface {
                    interface: actual,
                    span,
                    ..
                },
                Type::Interface(expected),
            ) => {
                self.diagnostics.push(mismatch(
                    &format!("interface {actual}"),
                    &format!("interface {expected}"),
                    span,
                    "interfaces do not implicitly convert to unrelated interfaces",
                ));
                None
            }
            (ObjectViewSource::Obj { span, .. }, Type::Interface(expected)) => {
                self.diagnostics.push(mismatch(
                    "Obj",
                    &format!("interface {expected}"),
                    span,
                    "an `Obj` view cannot convert implicitly to an interface",
                ));
                None
            }
            (
                source
                @ (ObjectViewSource::Produced { .. } | ObjectViewSource::ArrayElement { .. }),
                expected @ (Type::Class(_) | Type::Interface(_) | Type::Obj),
            ) => {
                let HirViewTarget::Class(class) = source.static_target() else {
                    unreachable!("exact retained sources must have a class target")
                };
                if required != HirAccess::ReadOnly {
                    self.diagnostics.push(
                        Diagnostic::error(
                            INVALID_ALIAS_ARGUMENT,
                            "mutable alias argument requires an existing object place",
                        )
                        .with_primary_label(
                            source_span,
                            "this source provides only a read-only retained view",
                        )
                        .with_secondary_label(parameter.span(), "mutable alias declared here"),
                    );
                    return None;
                }
                let expected_target = match expected {
                    Type::Class(class) => HirViewTarget::Class(class),
                    Type::Interface(interface) => HirViewTarget::Interface(interface),
                    Type::Obj => HirViewTarget::Obj,
                    _ => unreachable!(),
                };
                if !class_provides_view(self.program, class, expected_target) {
                    self.diagnostics.push(mismatch(
                        &view_target_name(self.program, HirViewTarget::Class(class)),
                        &view_target_name(self.program, expected_target),
                        source_span,
                        "this produced object cannot provide the required view",
                    ));
                    return None;
                }
                Some(HirCallArgument::View(
                    source.into_view_with_produced_projections(
                        expected_target,
                        HirAccess::ReadOnly,
                        static_class_up_projections(
                            self.program,
                            HirViewTarget::Class(class),
                            expected_target,
                        ),
                    ),
                ))
            }
            (
                ObjectViewSource::Shared(mut source),
                expected @ (Type::Class(_) | Type::Interface(_) | Type::Obj),
            ) => {
                let actual = source.static_target();
                let expected_target = match expected {
                    Type::Class(class) => HirViewTarget::Class(class),
                    Type::Interface(interface) => HirViewTarget::Interface(interface),
                    Type::Obj => HirViewTarget::Obj,
                    _ => unreachable!(),
                };
                if !crate::typeck::shared::target_accepts(
                    self.program,
                    super::object_view::view_shared_target(expected_target),
                    super::object_view::view_shared_target(actual),
                ) {
                    self.diagnostics.push(mismatch(
                        &view_target_name(self.program, actual),
                        &view_target_name(self.program, expected_target),
                        source_span,
                        "shared-backed aliases convert implicitly only to compatible up-views",
                    ));
                    return None;
                }
                let projections =
                    static_class_up_projections(self.program, actual, expected_target);
                source.set_projections(projections);
                Some(HirCallArgument::View(
                    source.into_view(expected_target, required),
                ))
            }
            (
                ObjectViewSource::Optional {
                    view,
                    class,
                    mut projections,
                },
                expected @ (Type::Class(_) | Type::Interface(_) | Type::Obj),
            ) => {
                let expected_target = match expected {
                    Type::Class(class) => HirViewTarget::Class(class),
                    Type::Interface(interface) => HirViewTarget::Interface(interface),
                    Type::Obj => HirViewTarget::Obj,
                    _ => unreachable!(),
                };
                if !class_provides_view(self.program, class, expected_target) {
                    self.diagnostics.push(mismatch(
                        &view_target_name(self.program, HirViewTarget::Class(class)),
                        &view_target_name(self.program, expected_target),
                        source_span,
                        "checked optional payload converts only to compatible up-views",
                    ));
                    return None;
                }
                projections.extend(static_class_up_projections(
                    self.program,
                    HirViewTarget::Class(class),
                    expected_target,
                ));
                Some(HirCallArgument::View(
                    ObjectViewSource::Optional {
                        view,
                        class,
                        projections,
                    }
                    .into_view(expected_target, required),
                ))
            }
            (
                source @ ObjectViewSource::OptionalBox { .. },
                expected @ (Type::Class(_) | Type::Interface(_) | Type::Obj),
            ) => {
                let actual = source.static_target();
                let expected_target = match expected {
                    Type::Class(class) => HirViewTarget::Class(class),
                    Type::Interface(interface) => HirViewTarget::Interface(interface),
                    Type::Obj => HirViewTarget::Obj,
                    _ => unreachable!(),
                };
                if !crate::typeck::shared::target_accepts(
                    self.program,
                    super::object_view::view_shared_target(expected_target),
                    super::object_view::view_shared_target(actual),
                ) {
                    self.diagnostics.push(mismatch(
                        &view_target_name(self.program, actual),
                        &view_target_name(self.program, expected_target),
                        source_span,
                        "boxed optional payload converts implicitly only to compatible up-views",
                    ));
                    return None;
                }
                Some(HirCallArgument::View(
                    source.into_view(expected_target, required),
                ))
            }
            (
                _,
                Type::I64
                | Type::U64
                | Type::U8
                | Type::F64
                | Type::Bool
                | Type::Unit
                | Type::Function(_)
                | Type::Optional(_)
                | Type::Array(_),
            ) => None,
            (_, Type::Shared(_)) => None,
        }
    }

    pub(super) fn project_place_to_ancestor(
        &self,
        mut place: HirObjectPlace,
        target: crate::identity::ClassId,
    ) -> Option<HirObjectPlace> {
        if place.class() == target {
            return Some(place);
        }
        let span = place.span();
        for base in self.program.hierarchy.base_chain(place.class())? {
            place.path = place.path.project_base(base, span);
            if base == target {
                return Some(place);
            }
        }
        None
    }
}

fn is_array_projection_through_groups(mut expression: &ResolvedExpression) -> bool {
    while let ResolvedExpression::Grouped(grouped) = expression {
        expression = &grouped.expression;
    }
    matches!(expression, ResolvedExpression::ArrayProjection(_))
}

fn view_target_name(program: &crate::resolve::ResolvedProgram, target: HirViewTarget) -> String {
    match target {
        HirViewTarget::Class(class) => program
            .class(class)
            .expect("view class must exist")
            .name
            .clone(),
        HirViewTarget::Interface(interface) => program
            .interface(interface)
            .expect("view interface must exist")
            .name
            .clone(),
        HirViewTarget::Obj => "Obj".to_owned(),
    }
}

fn static_class_up_projections(
    program: &crate::resolve::ResolvedProgram,
    actual: HirViewTarget,
    expected: HirViewTarget,
) -> Vec<crate::object_path::ObjectProjection> {
    let (HirViewTarget::Class(actual), HirViewTarget::Class(expected)) = (actual, expected) else {
        return Vec::new();
    };
    if actual == expected {
        return Vec::new();
    }
    program
        .hierarchy
        .base_chain(actual)
        .expect("compatible shared class view must have valid ancestry")
        .take_while(|class| *class != expected)
        .chain(std::iter::once(expected))
        .map(crate::object_path::ObjectProjection::Base)
        .collect()
}

const fn view_target_type(target: HirViewTarget) -> Type {
    match target {
        HirViewTarget::Class(class) => Type::Class(class),
        HirViewTarget::Interface(interface) => Type::Interface(interface),
        HirViewTarget::Obj => Type::Obj,
    }
}

fn forwarded_view(
    binding: BindingId,
    source_target: HirViewTarget,
    target: HirViewTarget,
    source_access: HirAccess,
    required_access: HirAccess,
    span: Span,
) -> HirCallArgument {
    HirCallArgument::View(forwarded_object_view(
        binding,
        source_target,
        target,
        source_access,
        required_access,
        span,
    ))
}

fn forwarded_object_view(
    binding: BindingId,
    source_target: HirViewTarget,
    target: HirViewTarget,
    source_access: HirAccess,
    required_access: HirAccess,
    span: Span,
) -> HirObjectView {
    HirObjectView {
        source: HirViewSource::Forwarded {
            binding,
            target: source_target,
            access: source_access,
            span,
        },
        origin: Box::new(HirObjectOrigin::Forwarded {
            binding,
            static_target: source_target,
            access: source_access,
            dispatch_limit: None,
            span,
        }),
        target,
        access: required_access,
        span,
    }
}
