//! Non-owning alias sources, access checks, and static view conversions.

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
        INVALID_NARROWING, INVALID_TYPE_TEST,
    },
};

#[derive(Clone, Copy)]
pub(super) enum ViewSourceUse {
    AliasArgument,
    TypeTest,
    Narrowing,
    Cast,
}

impl ViewSourceUse {
    const fn diagnostic_code(self) -> &'static str {
        match self {
            Self::AliasArgument => INVALID_ALIAS_ARGUMENT,
            Self::TypeTest => INVALID_TYPE_TEST,
            Self::Narrowing => INVALID_NARROWING,
            Self::Cast => crate::typeck::program::INVALID_OBJECT_CAST,
        }
    }

    const fn object_message(self) -> &'static str {
        match self {
            Self::AliasArgument => "alias argument must designate an object",
            Self::TypeTest => "type-test source must designate an object",
            Self::Narrowing => "checked-narrowing source must designate an object",
            Self::Cast => "object-cast source must designate an object",
        }
    }

    const fn place_message(self) -> &'static str {
        match self {
            Self::AliasArgument => "alias argument must be an existing object place",
            Self::TypeTest => "type-test source must be an existing object place",
            Self::Narrowing => "checked-narrowing source must be an existing object place",
            Self::Cast => "object-cast source must be an existing object place",
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
    Produced {
        source: crate::hir::HirObjectProducer,
        class: crate::identity::ClassId,
        span: Span,
    },
}

impl CheckedObjectViewSource {
    pub(super) const fn access(&self) -> HirAccess {
        match self {
            Self::Class { place, .. } => place.access,
            Self::Obj { access, .. } | Self::Interface { access, .. } => *access,
            Self::Produced { .. } => HirAccess::Mutable,
        }
    }

    pub(super) const fn span(&self) -> Span {
        match self {
            Self::Class { place, .. } => place.span(),
            Self::Obj { span, .. } | Self::Interface { span, .. } => *span,
            Self::Produced { span, .. } => *span,
        }
    }

    pub(super) const fn static_target(&self) -> HirViewTarget {
        match self {
            Self::Class { place, .. } => HirViewTarget::Class(place.class()),
            Self::Obj { .. } => HirViewTarget::Obj,
            Self::Interface { interface, .. } => HirViewTarget::Interface(*interface),
            Self::Produced { class, .. } => HirViewTarget::Class(*class),
        }
    }

    pub(super) fn exact_dynamic_class(&self) -> Option<crate::identity::ClassId> {
        match self {
            Self::Class {
                origin: HirObjectOrigin::Exact { dynamic_class, .. },
                ..
            } => Some(*dynamic_class),
            Self::Class {
                origin: HirObjectOrigin::Forwarded { .. } | HirObjectOrigin::Produced { .. },
                ..
            }
            | Self::Obj { .. }
            | Self::Interface { .. } => None,
            Self::Produced { class, .. } => Some(*class),
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
        }
    }
}

impl CallableChecker<'_, '_> {
    pub(super) fn check_alias_argument(
        &mut self,
        expression: &ResolvedExpression,
        parameter: &impl CallParameter,
    ) -> Option<HirCallArgument> {
        if let ResolvedExpression::ObjectCast(cast) = expression {
            return self.check_cast_alias_argument(cast, parameter);
        }
        if let ResolvedExpression::Grouped(grouped) = expression {
            if matches!(*grouped.expression, ResolvedExpression::ObjectCast(_)) {
                return self.check_alias_argument(&grouped.expression, parameter);
            }
        }
        let source = self.check_object_view_source(expression, ViewSourceUse::AliasArgument)?;
        let required = lower_parameter_mode(parameter.binding_mode())
            .required_access()
            .expect("alias parameter mode must require place access");
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

    pub(super) fn check_object_view_source(
        &mut self,
        expression: &ResolvedExpression,
        source_use: ViewSourceUse,
    ) -> Option<CheckedObjectViewSource> {
        match expression {
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
                }
                Some(source)
            }
            ResolvedExpression::FieldAccess(access) => {
                let field = self
                    .program
                    .field(access.field)
                    .expect("resolved field access must reference a field");
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
                let place = self.check_object_place(&place, ObjectPlaceUse::Alias)?;
                let origin = self.object_origin(&place);
                Some(CheckedObjectViewSource::Class { place, origin })
            }
            expression
                if matches!(source_use, ViewSourceUse::Cast)
                    && !is_object_cast_expression(expression)
                    && self.resolved_object_class(expression).is_some() =>
            {
                let class = self
                    .resolved_object_class(expression)
                    .expect("guarded produced object class");
                let source = self.check_object_source(expression, class, "object-cast source")?;
                let crate::hir::HirObjectSource::Produced(source) = source else {
                    unreachable!("non-place object cast source must produce an object")
                };
                Some(CheckedObjectViewSource::Produced {
                    span: expression.span(),
                    source,
                    class,
                })
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
                    "an `Obj` view cannot implicitly narrow to a class",
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
                    "an interface view cannot implicitly narrow to a class",
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
                    "an `Obj` view cannot implicitly narrow to an interface",
                ));
                None
            }
            (_, Type::I64 | Type::U64 | Type::U8 | Type::F64 | Type::Bool | Type::Unit) => None,
            (CheckedObjectViewSource::Produced { .. }, _) => {
                unreachable!("produced views enter alias arguments only through explicit casts")
            }
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
        HirObjectOrigin::Produced {
            span: origin_span, ..
        } => *origin_span = span,
    }
}
