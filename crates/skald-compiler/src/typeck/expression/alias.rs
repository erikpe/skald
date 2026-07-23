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
    },
};

enum CheckedAliasSource {
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
}

impl CheckedAliasSource {
    const fn access(&self) -> HirAccess {
        match self {
            Self::Class { place, .. } => place.access,
            Self::Obj { access, .. } | Self::Interface { access, .. } => *access,
        }
    }

    const fn span(&self) -> Span {
        match self {
            Self::Class { place, .. } => place.span(),
            Self::Obj { span, .. } | Self::Interface { span, .. } => *span,
        }
    }
}

impl CallableChecker<'_, '_> {
    pub(super) fn check_alias_argument(
        &mut self,
        expression: &ResolvedExpression,
        parameter: &impl CallParameter,
    ) -> Option<HirCallArgument> {
        let source = self.check_alias_source(expression)?;
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

    fn check_alias_source(
        &mut self,
        expression: &ResolvedExpression,
    ) -> Option<CheckedAliasSource> {
        match expression {
            ResolvedExpression::Binding(binding) => {
                if self.binding_type(binding.binding) == Type::Obj {
                    let access = self.binding_access(binding.binding, false, binding.span)?;
                    Some(CheckedAliasSource::Obj {
                        binding: binding.binding,
                        access,
                        span: binding.span,
                    })
                } else if let Type::Interface(interface) = self.binding_type(binding.binding) {
                    let access = self.binding_access(binding.binding, false, binding.span)?;
                    Some(CheckedAliasSource::Interface {
                        binding: binding.binding,
                        interface,
                        access,
                        span: binding.span,
                    })
                } else {
                    let place = self.check_binding_place(binding.binding, binding.span, false)?;
                    let origin = self.object_origin(&place);
                    Some(CheckedAliasSource::Class { place, origin })
                }
            }
            ResolvedExpression::Grouped(grouped) => {
                let mut source = self.check_alias_source(&grouped.expression)?;
                match &mut source {
                    CheckedAliasSource::Class { place, origin } => {
                        place.path.span = grouped.span;
                        set_origin_span(origin, grouped.span);
                    }
                    CheckedAliasSource::Obj { span, .. }
                    | CheckedAliasSource::Interface { span, .. } => *span = grouped.span,
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
                            INVALID_ALIAS_ARGUMENT,
                            "alias argument must designate an object",
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
                Some(CheckedAliasSource::Class { place, origin })
            }
            _ => {
                self.diagnostics.push(
                    Diagnostic::error(
                        INVALID_ALIAS_ARGUMENT,
                        "alias argument must be an existing object place",
                    )
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
        source: CheckedAliasSource,
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
            (CheckedAliasSource::Class { place, origin }, Type::Class(target)) => {
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
            (CheckedAliasSource::Class { place, origin }, Type::Obj) => {
                let span = place.span();
                Some(HirCallArgument::View(HirObjectView {
                    source: HirViewSource::Place(place),
                    origin: Box::new(origin),
                    target: HirViewTarget::Obj,
                    access: required,
                    span,
                }))
            }
            (CheckedAliasSource::Class { place, origin }, Type::Interface(interface)) => {
                let actual = place.class();
                if !self.class_conforms_to(actual, interface) {
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
                CheckedAliasSource::Obj {
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
            (CheckedAliasSource::Obj { span, .. }, Type::Class(target)) => {
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
                CheckedAliasSource::Interface {
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
                CheckedAliasSource::Interface {
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
            (CheckedAliasSource::Interface { span, .. }, Type::Class(target)) => {
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
                CheckedAliasSource::Interface {
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
            (CheckedAliasSource::Obj { span, .. }, Type::Interface(expected)) => {
                self.diagnostics.push(mismatch(
                    "Obj",
                    &format!("interface {expected}"),
                    span,
                    "an `Obj` view cannot implicitly narrow to an interface",
                ));
                None
            }
            (_, Type::I64 | Type::U64 | Type::U8 | Type::F64 | Type::Bool | Type::Unit) => None,
        }
    }

    fn class_conforms_to(
        &self,
        class: crate::identity::ClassId,
        interface: crate::identity::InterfaceId,
    ) -> bool {
        std::iter::once(class)
            .chain(
                self.program
                    .hierarchy
                    .base_chain(class)
                    .into_iter()
                    .flatten(),
            )
            .any(|candidate| {
                self.program.class(candidate).is_some_and(|declaration| {
                    declaration
                        .implemented_interfaces
                        .iter()
                        .any(|claim| claim.interface == interface)
                })
            })
    }

    fn project_place_to_ancestor(
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

fn forwarded_view(
    binding: BindingId,
    source_target: HirViewTarget,
    target: HirViewTarget,
    source_access: HirAccess,
    required_access: HirAccess,
    span: Span,
) -> HirCallArgument {
    HirCallArgument::View(HirObjectView {
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
    })
}

fn set_origin_span(origin: &mut HirObjectOrigin, span: Span) {
    match origin {
        HirObjectOrigin::Exact { complete, .. } => complete.path.span = span,
        HirObjectOrigin::Forwarded {
            span: origin_span, ..
        } => *origin_span = span,
    }
}
