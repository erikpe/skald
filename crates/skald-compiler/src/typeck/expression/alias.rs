//! Non-owning alias sources, access checks, and static view conversions.

use super::shared_pointee::CheckedSharedPointee;
use super::*;
use crate::{
    hir::{
        HirAccess, HirCallArgument, HirObjectOrigin, HirObjectPlace, HirObjectView, HirViewSource,
        HirViewTarget, Type,
    },
    identity::BindingId,
    resolve::{ResolvedExpression, ResolvedTypeKind},
    source::Span,
    typeck::program::{
        lower_parameter_mode, lower_type, INSUFFICIENT_ALIAS_ACCESS, INVALID_ALIAS_ARGUMENT,
        INVALID_COPY_CONSTRUCTION, INVALID_TYPE_TEST,
    },
};

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum ViewSourceUse {
    AliasArgument,
    TypeTest,
    Cast,
    CopyConstruction,
}

impl ViewSourceUse {
    const fn accepts_produced_inline(self) -> bool {
        matches!(
            self,
            Self::AliasArgument | Self::Cast | Self::CopyConstruction
        )
    }

    const fn source_context(self) -> &'static str {
        match self {
            Self::AliasArgument => "alias argument source",
            Self::TypeTest => "type-test source",
            Self::Cast => "object-cast source",
            Self::CopyConstruction => "copy-construction source",
        }
    }

    const fn diagnostic_code(self) -> &'static str {
        match self {
            Self::AliasArgument => INVALID_ALIAS_ARGUMENT,
            Self::TypeTest => INVALID_TYPE_TEST,
            Self::Cast => crate::typeck::program::INVALID_OBJECT_CAST,
            Self::CopyConstruction => INVALID_COPY_CONSTRUCTION,
        }
    }

    const fn object_message(self) -> &'static str {
        match self {
            Self::AliasArgument => "alias argument must designate an object",
            Self::TypeTest => "type-test source must designate an object",
            Self::Cast => "object-cast source must designate an object",
            Self::CopyConstruction => "copy-construction source must designate an object",
        }
    }

    const fn place_message(self) -> &'static str {
        match self {
            Self::AliasArgument => "alias argument must be an existing object place",
            Self::TypeTest => "type-test source must be an existing object place",
            Self::Cast => "object-cast source must be an existing object place",
            Self::CopyConstruction => {
                "copy-construction source must be an object place or produced object"
            }
        }
    }
}

pub(super) enum CheckedObjectViewSource {
    Class {
        place: HirObjectPlace,
        origin: HirObjectOrigin,
    },
    Obj {
        binding: BindingId,
        access: HirAccess,
        span: Span,
    },
    Interface {
        binding: BindingId,
        interface: crate::identity::InterfaceId,
        access: HirAccess,
        span: Span,
    },
    Shared(CheckedSharedPointee),
    Produced {
        source: crate::hir::HirObjectProducer,
        class: crate::identity::ClassId,
        span: Span,
    },
    Optional {
        view: crate::hir::HirCheckedOptionalView,
        class: crate::identity::ClassId,
        projections: Vec<crate::object_path::ObjectProjection>,
    },
}

impl CheckedObjectViewSource {
    pub(super) const fn access(&self) -> HirAccess {
        match self {
            Self::Class { place, .. } => place.access,
            Self::Obj { access, .. } | Self::Interface { access, .. } => *access,
            Self::Shared(source) => source.access(),
            Self::Produced { .. } => HirAccess::Mutable,
            Self::Optional { view, .. } => view.access,
        }
    }

    pub(super) const fn span(&self) -> Span {
        match self {
            Self::Class { place, .. } => place.span(),
            Self::Obj { span, .. } | Self::Interface { span, .. } => *span,
            Self::Shared(source) => source.span(),
            Self::Produced { span, .. } => *span,
            Self::Optional { view, .. } => view.span,
        }
    }

    pub(super) const fn static_target(&self) -> HirViewTarget {
        match self {
            Self::Class { place, .. } => HirViewTarget::Class(place.class()),
            Self::Obj { .. } => HirViewTarget::Obj,
            Self::Interface { interface, .. } => HirViewTarget::Interface(*interface),
            Self::Shared(source) => source.static_target(),
            Self::Produced { class, .. } => HirViewTarget::Class(*class),
            Self::Optional { class, .. } => HirViewTarget::Class(*class),
        }
    }

    pub(super) fn exact_dynamic_class(&self) -> Option<crate::identity::ClassId> {
        match self {
            Self::Class {
                origin: HirObjectOrigin::Exact { dynamic_class, .. },
                ..
            } => Some(*dynamic_class),
            Self::Class {
                origin:
                    HirObjectOrigin::Forwarded { .. }
                    | HirObjectOrigin::Shared { .. }
                    | HirObjectOrigin::AnchoredShared { .. }
                    | HirObjectOrigin::Produced { .. },
                ..
            }
            | Self::Obj { .. }
            | Self::Interface { .. } => None,
            Self::Shared(source) => source.exact_dynamic_class(),
            Self::Produced { class, .. } => Some(*class),
            Self::Optional { class, .. } => Some(*class),
        }
    }

    pub(super) fn relation_source(&self) -> super::object_view_relation::ObjectViewSource {
        self.exact_dynamic_class().map_or_else(
            || super::object_view_relation::ObjectViewSource::Dynamic(self.static_target()),
            super::object_view_relation::ObjectViewSource::ExactClass,
        )
    }

    pub(super) fn into_view(self, target: HirViewTarget, access: HirAccess) -> HirObjectView {
        match self {
            Self::Class { place, origin } => HirObjectView {
                span: place.span(),
                source: HirViewSource::Place(place),
                origin: Box::new(origin),
                target,
                access,
            },
            Self::Obj {
                binding,
                access: source_access,
                span,
            } => forwarded_object_view(
                binding,
                HirViewTarget::Obj,
                target,
                source_access,
                access,
                span,
            ),
            Self::Interface {
                binding,
                interface,
                access: source_access,
                span,
            } => forwarded_object_view(
                binding,
                HirViewTarget::Interface(interface),
                target,
                source_access,
                access,
                span,
            ),
            Self::Produced {
                source,
                class,
                span,
            } => HirObjectView {
                source: HirViewSource::Produced(Box::new(source)),
                origin: Box::new(HirObjectOrigin::Produced {
                    dynamic_class: class,
                    span,
                }),
                target,
                access,
                span,
            },
            Self::Shared(source) => source.into_view(target, access),
            Self::Optional {
                view,
                class,
                projections,
            } => {
                let span = view.span;
                HirObjectView {
                    source: HirViewSource::OptionalPayload {
                        view: Box::new(view),
                        projections,
                    },
                    origin: Box::new(HirObjectOrigin::Produced {
                        dynamic_class: class,
                        span,
                    }),
                    target,
                    access,
                    span,
                }
            }
        }
    }
}

impl CallableChecker<'_, '_> {
    pub(super) fn check_alias_argument(
        &mut self,
        expression: &ResolvedExpression,
        parameter: &impl CallParameter,
    ) -> Option<HirCallArgument> {
        let expected = lower_type(parameter.type_syntax());
        if matches!(expected, Type::Array(_)) || is_array_projection_through_groups(expression) {
            return self.check_array_alias_argument(expression, parameter);
        }
        if matches!(
            expected,
            Type::I64 | Type::U64 | Type::U8 | Type::F64 | Type::Bool
        ) {
            return self.check_primitive_alias_argument(expression, expected, parameter);
        }
        if matches!(
            expected,
            Type::OptionalPrimitive(_) | Type::OptionalClass(_)
        ) {
            return self.check_optional_alias_argument(expression, expected, parameter);
        }
        if let Some(target) = self.resolved_shared_target(expression) {
            let diagnostic = self
                .implicit_shared_dereference_diagnostic(expression.span(), target)
                .with_secondary_label(parameter.span(), "alias parameter declared here")
                .with_note(ViewSourceUse::AliasArgument.place_message());
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
        let source = self.check_object_view_source(expression, ViewSourceUse::AliasArgument)?;
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
            lower_type(parameter.type_syntax()),
            required,
            parameter,
        )
    }

    fn check_primitive_alias_argument(
        &mut self,
        expression: &ResolvedExpression,
        expected: Type,
        parameter: &impl CallParameter,
    ) -> Option<HirCallArgument> {
        let Some((place, actual, access)) = self.primitive_alias_place(expression) else {
            self.diagnostics.push(
                Diagnostic::error(
                    INVALID_ALIAS_ARGUMENT,
                    "primitive alias argument must designate an existing primitive place",
                )
                .with_primary_label(
                    expression.span(),
                    "pass a primitive binding or static field",
                )
                .with_secondary_label(parameter.span(), "primitive alias declared here"),
            );
            return None;
        };
        if actual != expected {
            self.diagnostics.push(
                Diagnostic::error(
                    TYPE_MISMATCH,
                    format!(
                        "primitive alias argument has type `{}` but `{}` is required",
                        actual.name(),
                        expected.name()
                    ),
                )
                .with_primary_label(place.span, "this place has a different primitive type")
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
        Some(HirCallArgument::PrimitivePlace(place))
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
                .map(|(place, ty)| (place, ty, HirAccess::Mutable)),
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
        let actual = match &place {
            crate::hir::HirOptionalAliasPlace::Primitive(place) => {
                Type::OptionalPrimitive(place.payload)
            }
            crate::hir::HirOptionalAliasPlace::Class(place) => Type::OptionalClass(place.class),
        };
        if actual != expected {
            self.diagnostics.push(
                Diagnostic::error(
                    TYPE_MISMATCH,
                    format!(
                        "optional alias argument has type `{}`, but `{}` is required",
                        actual.name(),
                        expected.name()
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
            crate::hir::HirOptionalStorage::Static(_) => Some(HirAccess::Mutable),
            crate::hir::HirOptionalStorage::Field(field) => Some(field.receiver.access),
            crate::hir::HirOptionalStorage::ArrayElement(place) => Some(place.receiver.access),
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
        let expected = lower_type(parameter.type_syntax());
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
                super::object_view_relation::class_provides_view(
                    self.program,
                    actual,
                    HirViewTarget::Interface(interface),
                )
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
            | ResolvedExpression::StaticCall(_)
            | ResolvedExpression::MethodCall(_)
            | ResolvedExpression::InterfaceCall(_) => {
                self.resolved_object_class(expression).is_some()
            }
            ResolvedExpression::Grouped(grouped) => {
                self.is_produced_alias_source(&grouped.expression)
            }
            ResolvedExpression::ObjectCast(cast) => self.is_produced_alias_source(&cast.source),
            _ => false,
        }
    }

    pub(super) fn check_object_view_source(
        &mut self,
        expression: &ResolvedExpression,
        source_use: ViewSourceUse,
    ) -> Option<CheckedObjectViewSource> {
        match expression {
            ResolvedExpression::Dereference(dereference) => self
                .check_explicit_shared_pointee(dereference, Vec::new(), dereference.span)
                .map(CheckedObjectViewSource::Shared),
            ResolvedExpression::Unwrap(unwrap) => {
                let view = self.check_class_optional_view(unwrap)?;
                let class = view.source.class();
                Some(CheckedObjectViewSource::Optional {
                    view,
                    class,
                    projections: Vec::new(),
                })
            }
            ResolvedExpression::Binding(binding) => {
                let binding_type = self.binding_type(binding.binding);
                if binding_type == Type::Obj {
                    let access = self.binding_access(binding.binding, false, binding.span)?;
                    Some(CheckedObjectViewSource::Obj {
                        binding: binding.binding,
                        access,
                        span: binding.span,
                    })
                } else if let Type::Interface(interface) = binding_type {
                    let access = self.binding_access(binding.binding, false, binding.span)?;
                    Some(CheckedObjectViewSource::Interface {
                        binding: binding.binding,
                        interface,
                        access,
                        span: binding.span,
                    })
                } else if matches!(binding_type, Type::Class(_)) {
                    let place = self.check_binding_place(binding.binding, binding.span, false)?;
                    let origin = self.object_origin(&place);
                    Some(CheckedObjectViewSource::Class { place, origin })
                } else if let Type::Shared(target) = binding_type {
                    self.reject_implicit_shared_view_source(
                        expression,
                        Type::Shared(target),
                        source_use,
                    )
                } else {
                    self.diagnostics.push(
                        Diagnostic::error(
                            source_use.diagnostic_code(),
                            source_use.object_message(),
                        )
                        .with_primary_label(binding.span, "this binding has a primitive type"),
                    );
                    None
                }
            }
            ResolvedExpression::Grouped(grouped) => {
                let mut source = self.check_object_view_source(&grouped.expression, source_use)?;
                match &mut source {
                    CheckedObjectViewSource::Class { place, origin } => {
                        place.path.span = grouped.span;
                        set_origin_span(origin, grouped.span);
                    }
                    CheckedObjectViewSource::Obj { span, .. }
                    | CheckedObjectViewSource::Interface { span, .. }
                    | CheckedObjectViewSource::Produced { span, .. } => *span = grouped.span,
                    CheckedObjectViewSource::Shared(source) => source.set_span(grouped.span),
                    CheckedObjectViewSource::Optional { view, .. } => view.span = grouped.span,
                }
                Some(source)
            }
            ResolvedExpression::FieldAccess(access) => {
                let field = self
                    .program
                    .field(access.field)
                    .expect("resolved field access must reference a field");
                if matches!(
                    access.receiver,
                    crate::resolve::ResolvedObjectReceiver::OptionalPayload { .. }
                ) {
                    let ResolvedTypeKind::Class(class) = field.type_syntax.kind else {
                        self.diagnostics.push(
                            Diagnostic::error(
                                source_use.diagnostic_code(),
                                source_use.object_message(),
                            )
                            .with_primary_label(
                                access.member_span,
                                "this field has a primitive type",
                            ),
                        );
                        return None;
                    };
                    let receiver =
                        self.check_object_receiver(&access.receiver, ObjectPlaceUse::Alias)?;
                    let optional = receiver
                        .optional_view
                        .expect("optional receiver must retain its checked payload view");
                    let HirViewSource::OptionalPayload {
                        view,
                        mut projections,
                    } = optional.source
                    else {
                        unreachable!("optional receiver must use optional payload provenance")
                    };
                    projections.push(crate::object_path::ObjectProjection::Field(access.field));
                    return Some(CheckedObjectViewSource::Optional {
                        view: *view,
                        class,
                        projections,
                    });
                }
                if matches!(field.type_syntax.kind, ResolvedTypeKind::Shared(_)) {
                    return self.reject_implicit_shared_view_source(
                        expression,
                        lower_type(&field.type_syntax),
                        source_use,
                    );
                }
                let ResolvedTypeKind::Class(class) = field.type_syntax.kind else {
                    self.diagnostics.push(
                        Diagnostic::error(
                            source_use.diagnostic_code(),
                            source_use.object_message(),
                        )
                        .with_primary_label(access.member_span, "this field has a primitive type"),
                    );
                    return None;
                };
                let place = access
                    .receiver
                    .clone()
                    .project_field(access.field, class, access.span);
                let Some(path) = place.binding_path() else {
                    self.diagnostics.push(
                        Diagnostic::error(
                            source_use.diagnostic_code(),
                            "a cast-relative field cannot be the source of another type operation",
                        )
                        .with_primary_label(
                            access.span,
                            "consume this checked field directly or copy it into inline storage",
                        ),
                    );
                    return None;
                };
                let place = self.check_object_place(path, ObjectPlaceUse::Alias)?;
                let origin = self.object_origin(&place);
                Some(CheckedObjectViewSource::Class { place, origin })
            }
            expression
                if self.resolved_shared_target(expression).is_some()
                    && matches!(
                        expression,
                        ResolvedExpression::Allocation(_)
                            | ResolvedExpression::DirectCall(_)
                            | ResolvedExpression::StaticCall(_)
                            | ResolvedExpression::MethodCall(_)
                            | ResolvedExpression::InterfaceCall(_)
                            | ResolvedExpression::ObjectCast(_)
                    ) =>
            {
                let target = self
                    .resolved_shared_target(expression)
                    .expect("guarded shared expression must retain its target");
                self.reject_implicit_shared_view_source(
                    expression,
                    Type::Shared(target),
                    source_use,
                )
            }
            expression
                if source_use.accepts_produced_inline()
                    && !is_object_cast_expression(expression)
                    && self.resolved_object_class(expression).is_some() =>
            {
                self.check_produced_inline_view_source(expression, source_use)
            }
            _ => {
                self.diagnostics.push(
                    Diagnostic::error(source_use.diagnostic_code(), source_use.place_message())
                        .with_primary_label(
                            expression.span(),
                            "expected an object local, `self`, alias parameter, or grouping",
                        ),
                );
                None
            }
        }
    }

    fn reject_implicit_shared_view_source(
        &mut self,
        expression: &ResolvedExpression,
        owner_type: Type,
        source_use: ViewSourceUse,
    ) -> Option<CheckedObjectViewSource> {
        let Type::Shared(target) = owner_type else {
            unreachable!("implicit shared view rejection requires a shared owner");
        };
        self.reject_implicit_shared_dereference(
            expression.span(),
            target,
            source_use.place_message(),
        )
    }

    fn check_produced_inline_view_source(
        &mut self,
        expression: &ResolvedExpression,
        source_use: ViewSourceUse,
    ) -> Option<CheckedObjectViewSource> {
        let Some(class) = self.resolved_object_class(expression) else {
            self.diagnostics.push(
                Diagnostic::error(source_use.diagnostic_code(), source_use.place_message())
                    .with_primary_label(
                        expression.span(),
                        "expected an object local, `self`, alias parameter, or grouping",
                    ),
            );
            return None;
        };
        let source = self.check_object_source(expression, class, source_use.source_context())?;
        let crate::hir::HirObjectSource::Produced(source) = source else {
            unreachable!("non-place object cast source must produce an object")
        };
        Some(CheckedObjectViewSource::Produced {
            span: expression.span(),
            source,
            class,
        })
    }

    fn convert_alias_argument(
        &mut self,
        source: CheckedObjectViewSource,
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
            (CheckedObjectViewSource::Class { place, origin }, Type::Class(target)) => {
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
            (CheckedObjectViewSource::Class { place, origin }, Type::Obj) => {
                let span = place.span();
                Some(HirCallArgument::View(HirObjectView {
                    source: HirViewSource::Place(place),
                    origin: Box::new(origin),
                    target: HirViewTarget::Obj,
                    access: required,
                    span,
                }))
            }
            (CheckedObjectViewSource::Class { place, origin }, Type::Interface(interface)) => {
                let actual = place.class();
                if !super::object_view_relation::class_provides_view(
                    self.program,
                    actual,
                    HirViewTarget::Interface(interface),
                ) {
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
                CheckedObjectViewSource::Obj {
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
            (CheckedObjectViewSource::Obj { span, .. }, Type::Class(target)) => {
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
                CheckedObjectViewSource::Interface {
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
                CheckedObjectViewSource::Interface {
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
            (CheckedObjectViewSource::Interface { span, .. }, Type::Class(target)) => {
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
                CheckedObjectViewSource::Interface {
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
            (CheckedObjectViewSource::Obj { span, .. }, Type::Interface(expected)) => {
                self.diagnostics.push(mismatch(
                    "Obj",
                    &format!("interface {expected}"),
                    span,
                    "an `Obj` view cannot convert implicitly to an interface",
                ));
                None
            }
            (
                source @ CheckedObjectViewSource::Produced { class, .. },
                expected @ (Type::Class(_) | Type::Interface(_) | Type::Obj),
            ) => {
                if required != HirAccess::ReadOnly {
                    self.diagnostics.push(
                        Diagnostic::error(
                            INVALID_ALIAS_ARGUMENT,
                            "mutable alias argument requires an existing object place",
                        )
                        .with_primary_label(
                            source_span,
                            "this expression produces a temporary object",
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
                if !super::object_view_relation::class_provides_view(
                    self.program,
                    class,
                    expected_target,
                ) {
                    self.diagnostics.push(mismatch(
                        &view_target_name(self.program, HirViewTarget::Class(class)),
                        &view_target_name(self.program, expected_target),
                        source_span,
                        "this produced object cannot provide the required view",
                    ));
                    return None;
                }
                Some(HirCallArgument::View(
                    source.into_view(expected_target, HirAccess::ReadOnly),
                ))
            }
            (
                CheckedObjectViewSource::Shared(mut source),
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
                    super::shared_pointee::view_shared_target(expected_target),
                    super::shared_pointee::view_shared_target(actual),
                ) {
                    self.diagnostics.push(mismatch(
                        &view_target_name(self.program, actual),
                        &view_target_name(self.program, expected_target),
                        source_span,
                        "shared-backed aliases convert implicitly only to compatible up-views",
                    ));
                    return None;
                }
                let projections = shared_up_projections(self.program, actual, expected_target);
                source.set_projections(projections);
                Some(HirCallArgument::View(
                    source.into_view(expected_target, required),
                ))
            }
            (
                CheckedObjectViewSource::Optional {
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
                if !super::object_view_relation::class_provides_view(
                    self.program,
                    class,
                    expected_target,
                ) {
                    self.diagnostics.push(mismatch(
                        &view_target_name(self.program, HirViewTarget::Class(class)),
                        &view_target_name(self.program, expected_target),
                        source_span,
                        "checked optional payload converts only to compatible up-views",
                    ));
                    return None;
                }
                projections.extend(shared_up_projections(
                    self.program,
                    HirViewTarget::Class(class),
                    expected_target,
                ));
                Some(HirCallArgument::View(
                    CheckedObjectViewSource::Optional {
                        view,
                        class,
                        projections,
                    }
                    .into_view(expected_target, required),
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
                | Type::OptionalPrimitive(_)
                | Type::OptionalClass(_)
                | Type::OptionalShared(_)
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

fn shared_up_projections(
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

fn is_object_cast_expression(expression: &ResolvedExpression) -> bool {
    match expression {
        ResolvedExpression::ObjectCast(_) => true,
        ResolvedExpression::Grouped(grouped) => is_object_cast_expression(&grouped.expression),
        _ => false,
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

fn set_origin_span(origin: &mut HirObjectOrigin, span: Span) {
    match origin {
        HirObjectOrigin::Exact { complete, .. } => complete.path.span = span,
        HirObjectOrigin::Forwarded {
            span: origin_span, ..
        } => *origin_span = span,
        HirObjectOrigin::Shared {
            span: origin_span, ..
        }
        | HirObjectOrigin::AnchoredShared {
            span: origin_span, ..
        } => *origin_span = span,
        HirObjectOrigin::Produced {
            span: origin_span, ..
        } => *origin_span = span,
    }
}
