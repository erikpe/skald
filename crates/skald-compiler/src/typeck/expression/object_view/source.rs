//! Checked sources for non-owning object views.

use crate::{
    diagnostics::Diagnostic,
    hir::{
        HirAccess, HirObjectOrigin, HirObjectPlace, HirObjectView, HirViewSource, HirViewTarget,
        Type,
    },
    identity::BindingId,
    resolve::{ResolvedExpression, ResolvedTypeKind},
    source::Span,
    typeck::{expression::ObjectPlaceUse, function::CallableChecker, program::lower_type},
};

use super::{CheckedSharedPointee, ObjectViewRelationSource};

/// The source families a consumer permits before target planning begins.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::typeck) enum ObjectViewSourceAdmission {
    ExistingObjectPlace,
    ExistingOrProducedObject,
}

impl ObjectViewSourceAdmission {
    pub(super) const fn accepts_produced_inline(self) -> bool {
        matches!(self, Self::ExistingOrProducedObject)
    }
}

/// Consumer-owned wording for failures encountered while checking a source.
///
/// Admission is passed separately so diagnostic phrasing cannot silently
/// widen the accepted source families.
#[derive(Clone, Copy)]
pub(in crate::typeck) struct ObjectViewSourceDiagnosticContext {
    source_context: &'static str,
    diagnostic_code: &'static str,
    object_message: &'static str,
    place_message: &'static str,
}

impl ObjectViewSourceDiagnosticContext {
    pub(in crate::typeck) const fn new(
        source_context: &'static str,
        diagnostic_code: &'static str,
        object_message: &'static str,
        place_message: &'static str,
    ) -> Self {
        Self {
            source_context,
            diagnostic_code,
            object_message,
            place_message,
        }
    }

    const fn source_context(self) -> &'static str {
        self.source_context
    }

    const fn diagnostic_code(self) -> &'static str {
        self.diagnostic_code
    }

    const fn object_message(self) -> &'static str {
        self.object_message
    }

    pub(in crate::typeck) const fn place_message(self) -> &'static str {
        self.place_message
    }
}

/// A source checked exactly once with the HIR provenance needed by consumers.
pub(in crate::typeck) enum ObjectViewSource {
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
    Static {
        place: crate::hir::HirStaticPlace,
        dynamic_class: crate::identity::ClassId,
        class: crate::identity::ClassId,
        projections: Vec<crate::object_path::ObjectProjection>,
        span: Span,
    },
    Shared(CheckedSharedPointee),
    Produced {
        source: crate::hir::HirObjectProducer,
        dynamic_class: crate::identity::ClassId,
        class: crate::identity::ClassId,
        projections: Vec<crate::object_path::ObjectProjection>,
        span: Span,
    },
    Optional {
        view: crate::hir::HirCheckedOptionalView,
        dynamic_class: crate::identity::ClassId,
        class: crate::identity::ClassId,
        projections: Vec<crate::object_path::ObjectProjection>,
    },
    OptionalBox {
        view: crate::hir::HirOptionalBoxObjectView,
        projections: Vec<crate::object_path::ObjectProjection>,
    },
    ArrayElement {
        element: Box<crate::hir::HirArrayElementPlace>,
        dynamic_class: crate::identity::ClassId,
        class: crate::identity::ClassId,
        span: Span,
    },
}

impl ObjectViewSource {
    pub(in crate::typeck) const fn access(&self) -> HirAccess {
        match self {
            Self::Class { place, .. } => place.access,
            Self::Obj { access, .. } | Self::Interface { access, .. } => *access,
            Self::Static { .. } => HirAccess::Mutable,
            Self::Shared(source) => source.access(),
            Self::Produced { .. } => HirAccess::ReadOnly,
            Self::Optional { view, .. } => view.access,
            Self::OptionalBox { view, .. } => view.access,
            Self::ArrayElement { element, .. } => element.receiver.access,
        }
    }

    pub(in crate::typeck) const fn span(&self) -> Span {
        match self {
            Self::Class { place, .. } => place.span(),
            Self::Obj { span, .. } | Self::Interface { span, .. } => *span,
            Self::Static { span, .. } => *span,
            Self::Shared(source) => source.span(),
            Self::Produced { span, .. } => *span,
            Self::Optional { view, .. } => view.span,
            Self::OptionalBox { view, .. } => view.span,
            Self::ArrayElement { span, .. } => *span,
        }
    }

    pub(in crate::typeck) const fn static_target(&self) -> HirViewTarget {
        match self {
            Self::Class { place, .. } => HirViewTarget::Class(place.class()),
            Self::Obj { .. } => HirViewTarget::Obj,
            Self::Interface { interface, .. } => HirViewTarget::Interface(*interface),
            Self::Static { class, .. } => HirViewTarget::Class(*class),
            Self::Shared(source) => source.static_target(),
            Self::Produced { class, .. } => HirViewTarget::Class(*class),
            Self::Optional { class, .. } => HirViewTarget::Class(*class),
            Self::OptionalBox { view, .. } => view.target,
            Self::ArrayElement { class, .. } => HirViewTarget::Class(*class),
        }
    }

    pub(in crate::typeck) fn exact_dynamic_class(&self) -> Option<crate::identity::ClassId> {
        match self {
            Self::Class {
                origin:
                    HirObjectOrigin::Exact { dynamic_class, .. }
                    | HirObjectOrigin::Static { dynamic_class, .. },
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
            Self::Static { dynamic_class, .. } => Some(*dynamic_class),
            Self::Shared(source) => source.exact_dynamic_class(),
            Self::Produced { dynamic_class, .. } => Some(*dynamic_class),
            Self::Optional { dynamic_class, .. } => Some(*dynamic_class),
            Self::OptionalBox { view, .. } => view.source.exact_dynamic_class(),
            Self::ArrayElement { dynamic_class, .. } => Some(*dynamic_class),
        }
    }

    pub(in crate::typeck) fn relation_source(&self) -> ObjectViewRelationSource {
        self.exact_dynamic_class().map_or_else(
            || ObjectViewRelationSource::Dynamic(self.static_target()),
            ObjectViewRelationSource::ExactClass,
        )
    }

    pub(super) fn into_view(self, target: HirViewTarget, access: HirAccess) -> HirObjectView {
        self.into_view_with_target_projections(target, access, Vec::new())
    }

    pub(super) fn into_view_with_target_projections(
        self,
        target: HirViewTarget,
        access: HirAccess,
        target_projections: Vec<crate::object_path::ObjectProjection>,
    ) -> HirObjectView {
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
            Self::Static {
                place,
                dynamic_class,
                class: _,
                mut projections,
                span,
            } => {
                projections.extend(target_projections);
                HirObjectView {
                    source: HirViewSource::Static { place, projections },
                    origin: Box::new(HirObjectOrigin::Static {
                        place,
                        dynamic_class,
                    }),
                    target,
                    access,
                    span,
                }
            }
            Self::Produced {
                source,
                dynamic_class,
                class: _,
                mut projections,
                span,
            } => {
                projections.extend(target_projections);
                HirObjectView {
                    source: HirViewSource::Produced {
                        producer: Box::new(source),
                        projections,
                    },
                    origin: Box::new(HirObjectOrigin::Produced {
                        dynamic_class,
                        span,
                    }),
                    target,
                    access,
                    span,
                }
            }
            Self::Shared(source) => source.into_view(target, access),
            Self::Optional {
                view,
                dynamic_class,
                class: _,
                projections,
            } => {
                let span = view.span;
                HirObjectView {
                    source: HirViewSource::OptionalPayload {
                        view: Box::new(view),
                        projections,
                    },
                    origin: Box::new(HirObjectOrigin::Produced {
                        dynamic_class,
                        span,
                    }),
                    target,
                    access,
                    span,
                }
            }
            Self::OptionalBox { view, projections } => {
                super::super::optional_box_view::into_object_view(view, target, access, projections)
            }
            Self::ArrayElement {
                element,
                dynamic_class,
                class: _,
                span,
            } => HirObjectView {
                source: HirViewSource::ArrayElement(element),
                origin: Box::new(HirObjectOrigin::Produced {
                    dynamic_class,
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
    pub(in crate::typeck) fn check_object_view_source(
        &mut self,
        expression: &ResolvedExpression,
        admission: ObjectViewSourceAdmission,
        diagnostics: ObjectViewSourceDiagnosticContext,
    ) -> Option<ObjectViewSource> {
        match expression {
            ResolvedExpression::Dereference(dereference) => self
                .check_explicit_shared_pointee(dereference, Vec::new(), dereference.span)
                .map(ObjectViewSource::Shared),
            ResolvedExpression::Unwrap(unwrap) => {
                if let Some(view) = self.check_optional_box_object_view(unwrap) {
                    return Some(ObjectViewSource::OptionalBox {
                        view,
                        projections: Vec::new(),
                    });
                }
                let view = self.check_class_optional_view(unwrap)?;
                let class = self.optional_operand_class(&view.source);
                Some(ObjectViewSource::Optional {
                    view,
                    dynamic_class: class,
                    class,
                    projections: Vec::new(),
                })
            }
            ResolvedExpression::Binding(binding) => {
                let binding_type = self.binding_type(binding.binding);
                if binding_type == Type::Obj {
                    let access = self.binding_access(binding.binding, false, binding.span)?;
                    Some(ObjectViewSource::Obj {
                        binding: binding.binding,
                        access,
                        span: binding.span,
                    })
                } else if let Type::Interface(interface) = binding_type {
                    let access = self.binding_access(binding.binding, false, binding.span)?;
                    Some(ObjectViewSource::Interface {
                        binding: binding.binding,
                        interface,
                        access,
                        span: binding.span,
                    })
                } else if matches!(binding_type, Type::Class(_)) {
                    let place = self.check_binding_place(binding.binding, binding.span, false)?;
                    let origin = self.object_origin(&place);
                    Some(ObjectViewSource::Class { place, origin })
                } else if let Type::Shared(target) = binding_type {
                    self.reject_implicit_shared_view_source(
                        expression,
                        Type::Shared(target),
                        diagnostics,
                    )
                } else {
                    self.diagnostics.push(
                        Diagnostic::error(
                            diagnostics.diagnostic_code(),
                            diagnostics.object_message(),
                        )
                        .with_primary_label(binding.span, "this binding has a primitive type"),
                    );
                    None
                }
            }
            ResolvedExpression::Grouped(grouped) => {
                let mut source =
                    self.check_object_view_source(&grouped.expression, admission, diagnostics)?;
                match &mut source {
                    ObjectViewSource::Class { place, origin } => {
                        place.path.span = grouped.span;
                        set_origin_span(origin, grouped.span);
                    }
                    ObjectViewSource::Obj { span, .. }
                    | ObjectViewSource::Interface { span, .. }
                    | ObjectViewSource::Static { span, .. }
                    | ObjectViewSource::Produced { span, .. } => *span = grouped.span,
                    ObjectViewSource::Shared(source) => source.set_span(grouped.span),
                    ObjectViewSource::Optional { view, .. } => view.span = grouped.span,
                    ObjectViewSource::OptionalBox { view, .. } => view.span = grouped.span,
                    ObjectViewSource::ArrayElement { span, .. } => *span = grouped.span,
                }
                Some(source)
            }
            ResolvedExpression::ArrayProjection(projection) => {
                let checked = self.check_array_projection(projection)?;
                let Type::Class(class) = checked.ty else {
                    self.diagnostics.push(
                        Diagnostic::error(
                            diagnostics.diagnostic_code(),
                            diagnostics.object_message(),
                        )
                        .with_primary_label(checked.span, "this array element is not a class"),
                    );
                    return None;
                };
                let crate::hir::HirExpressionKind::ArrayElement(mut element) = checked.kind else {
                    unreachable!("checked indexed class source must retain its element place")
                };
                if element.receiver.ownership == crate::hir::HirArrayReceiverOwnership::Inline {
                    element.receiver.anchor = crate::hir::HirArrayAnchor::InlineBacking;
                }
                Some(ObjectViewSource::ArrayElement {
                    element,
                    dynamic_class: class,
                    class,
                    span: checked.span,
                })
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
                                diagnostics.diagnostic_code(),
                                diagnostics.object_message(),
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
                    let super::super::CheckedReceiverCarrier::View { view: optional, .. } =
                        receiver.carrier
                    else {
                        unreachable!("optional receiver must retain its checked payload view")
                    };
                    let HirViewSource::OptionalPayload {
                        view,
                        mut projections,
                    } = optional.source
                    else {
                        unreachable!("optional receiver must use optional payload provenance")
                    };
                    projections.push(crate::object_path::ObjectProjection::Field(access.field));
                    return Some(ObjectViewSource::Optional {
                        view: *view,
                        dynamic_class: class,
                        class,
                        projections,
                    });
                }
                if matches!(field.type_syntax.kind, ResolvedTypeKind::Shared(_)) {
                    return self.reject_implicit_shared_view_source(
                        expression,
                        lower_type(self.program, &field.type_syntax),
                        diagnostics,
                    );
                }
                let ResolvedTypeKind::Class(class) = field.type_syntax.kind else {
                    self.diagnostics.push(
                        Diagnostic::error(
                            diagnostics.diagnostic_code(),
                            diagnostics.object_message(),
                        )
                        .with_primary_label(access.member_span, "this field has a primitive type"),
                    );
                    return None;
                };
                if matches!(
                    access.receiver,
                    crate::resolve::ResolvedObjectReceiver::Produced { .. }
                ) {
                    let receiver =
                        access
                            .receiver
                            .clone()
                            .project_field(access.field, class, access.span);
                    let checked = self.check_object_receiver(&receiver, ObjectPlaceUse::Alias)?;
                    let super::super::CheckedReceiverCarrier::View { view, .. } = checked.carrier
                    else {
                        unreachable!("produced field source must retain its object view")
                    };
                    let HirViewSource::Produced {
                        producer,
                        projections,
                    } = view.source
                    else {
                        unreachable!("produced field source must retain produced provenance")
                    };
                    let HirObjectOrigin::Produced { dynamic_class, .. } = *view.origin else {
                        unreachable!("produced field source must retain exact dynamic class")
                    };
                    let HirViewTarget::Class(static_class) = view.target else {
                        unreachable!("produced field source must retain a class target")
                    };
                    debug_assert_eq!(dynamic_class, class);
                    return Some(ObjectViewSource::Produced {
                        source: *producer,
                        dynamic_class,
                        class: static_class,
                        projections,
                        span: access.span,
                    });
                }
                let place = access
                    .receiver
                    .clone()
                    .project_field(access.field, class, access.span);
                let Some(path) = place.binding_path() else {
                    self.diagnostics.push(
                        Diagnostic::error(
                            diagnostics.diagnostic_code(),
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
                Some(ObjectViewSource::Class { place, origin })
            }
            expression
                if self.resolved_shared_target(expression).is_some()
                    && matches!(
                        expression,
                        ResolvedExpression::Allocation(_)
                            | ResolvedExpression::DirectCall(_)
                            | ResolvedExpression::IndirectCall(_)
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
                    diagnostics,
                )
            }
            expression
                if admission.accepts_produced_inline()
                    && !is_object_cast_expression(expression)
                    && self.resolved_object_class(expression).is_some() =>
            {
                self.check_produced_inline_view_source(expression, diagnostics)
            }
            _ => {
                self.diagnostics.push(
                    Diagnostic::error(diagnostics.diagnostic_code(), diagnostics.place_message())
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
        diagnostics: ObjectViewSourceDiagnosticContext,
    ) -> Option<ObjectViewSource> {
        let Type::Shared(target) = owner_type else {
            unreachable!("implicit shared view rejection requires a shared owner");
        };
        self.reject_implicit_shared_dereference(
            expression.span(),
            target,
            diagnostics.place_message(),
        )
    }

    fn check_produced_inline_view_source(
        &mut self,
        expression: &ResolvedExpression,
        diagnostics: ObjectViewSourceDiagnosticContext,
    ) -> Option<ObjectViewSource> {
        let Some(class) = self.resolved_object_class(expression) else {
            self.diagnostics.push(
                Diagnostic::error(diagnostics.diagnostic_code(), diagnostics.place_message())
                    .with_primary_label(
                        expression.span(),
                        "expected an object local, `self`, alias parameter, or grouping",
                    ),
            );
            return None;
        };
        let source = self.check_object_source(expression, class, diagnostics.source_context())?;
        let crate::hir::HirObjectSource::Produced(source) = source else {
            unreachable!("non-place object cast source must produce an object")
        };
        Some(ObjectViewSource::Produced {
            span: expression.span(),
            source,
            dynamic_class: class,
            class,
            projections: Vec::new(),
        })
    }
}

fn is_object_cast_expression(expression: &ResolvedExpression) -> bool {
    match expression {
        ResolvedExpression::ObjectCast(_) => true,
        ResolvedExpression::Grouped(grouped) => is_object_cast_expression(&grouped.expression),
        _ => false,
    }
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
        HirObjectOrigin::Static { place, .. } => place.span = span,
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
