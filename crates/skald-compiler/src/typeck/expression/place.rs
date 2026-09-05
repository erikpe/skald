//! Typed inline-object places, root capabilities, and initializer liveness.

use crate::{
    diagnostics::Diagnostic,
    hir::{
        HirAccess, HirCheckedObjectView, HirExpression, HirExpressionKind, HirFieldPlace,
        HirObjectOrigin, HirObjectPlace, HirObjectReceiver, HirObjectView, Type,
    },
    identity::{BindingId, FieldId, ParameterId},
    object_path::{ObjectPath, ObjectProjection},
    resolve::{
        ResolvedExpression, ResolvedObjectPlace, ResolvedObjectReceiver, ResolvedParameter,
        ResolvedTypeKind,
    },
    source::Span,
};

use super::super::{
    function::{CallableChecker, MemberBodyKind},
    program::{
        lower_parameter_mode, lower_type, FIELD_INITIALIZATION, INVALID_ALIAS_ARGUMENT,
        INVALID_OBJECT_CONTEXT,
    },
};

#[derive(Clone, Copy)]
pub(in crate::typeck) enum ObjectPlaceUse {
    Member,
    Alias,
    CopySource,
    CopyDestination,
    InitializationDestination,
}

pub(in crate::typeck) struct CheckedObjectReceiver {
    pub origin: HirObjectOrigin,
    pub carrier: CheckedReceiverCarrier,
}

pub(in crate::typeck) enum CheckedReceiverCarrier {
    Place(HirObjectPlace),
    Checked {
        place: HirObjectPlace,
        view: Box<HirCheckedObjectView>,
    },
    View {
        view: Box<HirObjectView>,
        inspection_place: Option<Box<HirObjectPlace>>,
    },
    ArrayElement {
        element: Box<crate::hir::HirArrayElementPlace>,
        place: HirObjectPlace,
    },
}

impl CheckedObjectReceiver {
    pub(in crate::typeck) fn access(&self) -> HirAccess {
        match &self.carrier {
            CheckedReceiverCarrier::Place(place)
            | CheckedReceiverCarrier::Checked { place, .. }
            | CheckedReceiverCarrier::ArrayElement { place, .. } => place.access,
            CheckedReceiverCarrier::View { view, .. } => view.access,
        }
    }

    pub(in crate::typeck) fn class(&self) -> crate::identity::ClassId {
        match &self.carrier {
            CheckedReceiverCarrier::Place(place)
            | CheckedReceiverCarrier::Checked { place, .. }
            | CheckedReceiverCarrier::ArrayElement { place, .. } => place.class(),
            CheckedReceiverCarrier::View { view, .. } => match view.target {
                crate::hir::HirViewTarget::Class(class) => class,
                crate::hir::HirViewTarget::Interface(_) | crate::hir::HirViewTarget::Obj => {
                    unreachable!("ordinary object receivers require a class view")
                }
            },
        }
    }

    pub(in crate::typeck) fn inspection_place(&self) -> Option<&HirObjectPlace> {
        match &self.carrier {
            CheckedReceiverCarrier::Place(place)
            | CheckedReceiverCarrier::Checked { place, .. }
            | CheckedReceiverCarrier::ArrayElement { place, .. } => Some(place),
            CheckedReceiverCarrier::View {
                inspection_place, ..
            } => inspection_place.as_deref(),
        }
    }

    pub(in crate::typeck) fn into_hir(self) -> HirObjectReceiver {
        match self.carrier {
            CheckedReceiverCarrier::Place(place) => HirObjectReceiver::Place {
                place,
                origin: Box::new(self.origin),
            },
            CheckedReceiverCarrier::Checked { place, view } => HirObjectReceiver::Checked {
                place,
                origin: Box::new(self.origin),
                view,
            },
            CheckedReceiverCarrier::View {
                view,
                inspection_place,
            } => HirObjectReceiver::View {
                view,
                inspection_place,
            },
            CheckedReceiverCarrier::ArrayElement { element, place } => {
                HirObjectReceiver::ArrayElement {
                    element,
                    place,
                    origin: Box::new(self.origin),
                }
            }
        }
    }
}

impl CallableChecker<'_, '_> {
    pub(in crate::typeck) fn check_field_read(
        &mut self,
        access: &crate::resolve::ResolvedFieldAccessExpr,
    ) -> Option<HirExpression> {
        let place = self.check_field_place(
            &access.receiver,
            access.field,
            access.span,
            ObjectPlaceUse::Member,
        )?;
        if !matches!(place.receiver, HirObjectReceiver::Checked { .. })
            && place.receiver.inspection_place().is_some_and(|receiver| {
                receiver.root() == BindingId::Receiver(self.callable) && receiver.path.is_root()
            })
            && !self.check_initializer_field_liveness(place.field, access.member_span)
        {
            return None;
        }
        let field = self
            .program
            .field(place.field)
            .expect("selected field must exist");
        if matches!(field.type_syntax.kind, ResolvedTypeKind::Class(_)) {
            self.diagnostics.push(
                Diagnostic::error(
                    INVALID_OBJECT_CONTEXT,
                    format!("class field `{}` is not a value", field.name),
                )
                .with_primary_label(
                    access.member_span,
                    "use this object place as a receiver or alias argument",
                ),
            );
            return None;
        }
        Some(HirExpression {
            kind: HirExpressionKind::FieldRead(place),
            ty: lower_type(self.program, &field.type_syntax),
            span: access.span,
        })
    }

    pub(in crate::typeck) fn check_field_place(
        &mut self,
        receiver: &ResolvedObjectReceiver,
        field: FieldId,
        span: Span,
        place_use: ObjectPlaceUse,
    ) -> Option<HirFieldPlace> {
        let checked = self.check_object_receiver(receiver, place_use)?;
        Some(HirFieldPlace {
            receiver: checked.into_hir(),
            field,
            write_authorization: None,
            span,
        })
    }

    pub(in crate::typeck) fn check_object_receiver(
        &mut self,
        receiver: &ResolvedObjectReceiver,
        place_use: ObjectPlaceUse,
    ) -> Option<CheckedObjectReceiver> {
        if let ResolvedObjectReceiver::Produced {
            producer,
            exact_class,
            projections,
            class,
            span,
        } = receiver
        {
            let source =
                self.check_object_source(producer, *exact_class, "produced member receiver")?;
            let crate::hir::HirObjectSource::Produced(producer) = source else {
                unreachable!("resolved produced receiver must retain one object producer")
            };
            let dynamic_class = self.produced_projection_dynamic_class(*exact_class, projections);
            let origin = HirObjectOrigin::Produced {
                dynamic_class,
                span: *span,
            };
            let view = HirObjectView {
                source: crate::hir::HirViewSource::Produced {
                    producer: Box::new(producer),
                    projections: projections.clone(),
                },
                origin: Box::new(origin.clone()),
                target: crate::hir::HirViewTarget::Class(*class),
                access: HirAccess::ReadOnly,
                span: *span,
            };
            return Some(CheckedObjectReceiver {
                origin,
                carrier: CheckedReceiverCarrier::View {
                    view: Box::new(view),
                    inspection_place: None,
                },
            });
        }
        if let ResolvedObjectReceiver::StaticField {
            field,
            projections,
            class,
            span,
        } = receiver
        {
            let declaration = self
                .program
                .static_field(*field)
                .expect("resolved static receiver must reference a static field");
            let Type::Class(dynamic_class) = lower_type(self.program, &declaration.type_syntax)
            else {
                unreachable!("resolved static object receiver must retain an exact class type")
            };
            let place = crate::hir::HirStaticPlace {
                field: *field,
                span: *span,
            };
            let origin = HirObjectOrigin::Static {
                place,
                dynamic_class,
            };
            let view = HirObjectView {
                source: crate::hir::HirViewSource::Static {
                    place,
                    projections: projections.clone(),
                },
                origin: Box::new(origin.clone()),
                target: crate::hir::HirViewTarget::Class(*class),
                access: HirAccess::Mutable,
                span: *span,
            };
            return Some(CheckedObjectReceiver {
                origin,
                carrier: CheckedReceiverCarrier::View {
                    view: Box::new(view),
                    inspection_place: None,
                },
            });
        }
        if let ResolvedObjectReceiver::Dereference {
            dereference,
            projections,
            class,
            span,
        } = receiver
        {
            let pointee =
                self.check_explicit_shared_pointee(dereference, projections.clone(), *span)?;
            return Some(self.finish_shared_object_receiver(pointee, *class, *span));
        }
        if let ResolvedObjectReceiver::OptionalPayload {
            unwrap,
            projections,
            class,
            span,
        } = receiver
        {
            if let Some(view) = self.check_optional_box_object_view(unwrap) {
                let access = view.access;
                let optional_view = super::optional_box_view::into_object_view(
                    view,
                    crate::hir::HirViewTarget::Class(*class),
                    access,
                    projections.clone(),
                );
                let place = HirObjectPlace {
                    path: crate::object_path::ObjectPath {
                        root: BindingId::Receiver(self.callable),
                        projections: Vec::new(),
                        class: *class,
                        span: *span,
                    },
                    access,
                };
                return Some(CheckedObjectReceiver {
                    origin: (*optional_view.origin).clone(),
                    carrier: CheckedReceiverCarrier::View {
                        view: Box::new(optional_view),
                        inspection_place: Some(Box::new(place)),
                    },
                });
            }
            let view = self.check_class_optional_view(unwrap)?;
            let access = view.access;
            let root_class = self.optional_operand_class(&view.source);
            let source = crate::hir::HirViewSource::OptionalPayload {
                view: Box::new(view),
                projections: projections.clone(),
            };
            let optional_view = crate::hir::HirObjectView {
                source,
                origin: Box::new(HirObjectOrigin::Produced {
                    dynamic_class: root_class,
                    span: *span,
                }),
                target: crate::hir::HirViewTarget::Class(*class),
                access,
                span: *span,
            };
            let place = HirObjectPlace {
                path: crate::object_path::ObjectPath {
                    root: BindingId::Receiver(self.callable),
                    projections: Vec::new(),
                    class: *class,
                    span: *span,
                },
                access,
            };
            return Some(CheckedObjectReceiver {
                origin: HirObjectOrigin::Produced {
                    dynamic_class: root_class,
                    span: *span,
                },
                carrier: CheckedReceiverCarrier::View {
                    view: Box::new(optional_view),
                    inspection_place: Some(Box::new(place)),
                },
            });
        }
        if let ResolvedObjectReceiver::ArrayElement {
            projection,
            projections,
            class,
            span,
        } = receiver
        {
            let checked = self.check_array_projection(projection)?;
            let Type::Class(element_class) = checked.ty else {
                unreachable!("resolved array-element receiver must retain its class type")
            };
            let HirExpressionKind::ArrayElement(mut element) = checked.kind else {
                unreachable!("array-element receiver must be an indexed projection")
            };
            if element.receiver.ownership == crate::hir::HirArrayReceiverOwnership::Inline {
                element.receiver.anchor = crate::hir::HirArrayAnchor::InlineBacking;
            }
            let access = element.receiver.access;
            let place = HirObjectPlace {
                path: ObjectPath {
                    root: BindingId::Receiver(self.callable),
                    projections: projections.clone(),
                    class: *class,
                    span: *span,
                },
                access,
            };
            let origin = HirObjectOrigin::Exact {
                complete: HirObjectPlace {
                    path: ObjectPath {
                        root: BindingId::Receiver(self.callable),
                        projections: Vec::new(),
                        class: element_class,
                        span: element.span,
                    },
                    access,
                },
                dynamic_class: element_class,
            };
            return Some(CheckedObjectReceiver {
                origin,
                carrier: CheckedReceiverCarrier::ArrayElement { element, place },
            });
        }
        let ResolvedObjectReceiver::CastRelative {
            cast,
            projections,
            class,
            span,
        } = receiver
        else {
            let path = receiver
                .binding_path()
                .expect("ordinary receiver must retain its binding path");
            let place = self.check_object_place(path, place_use)?;
            let origin = self.object_origin(&place);
            return Some(CheckedObjectReceiver {
                origin,
                carrier: CheckedReceiverCarrier::Place(place),
            });
        };
        let mut checked = self.check_object_cast(cast)?;
        let target_class = checked
            .class
            .expect("class member receivers require a class cast target");
        checked.projections.extend_from_slice(projections);
        checked.class = Some(*class);
        // Preserve the historical inspection path beside the checked carrier.
        // Lowering selects the checked view and never treats this root as
        // executable provenance.
        let place = HirObjectPlace {
            path: crate::object_path::ObjectPath {
                root: BindingId::Receiver(self.callable),
                projections: projections.clone(),
                class: *class,
                span: *span,
            },
            access: checked.view.access,
        };
        debug_assert!(
            target_class == *class || !projections.is_empty(),
            "unprojected cast receiver must retain its target class"
        );
        let origin = (*checked.view.origin).clone();
        Some(CheckedObjectReceiver {
            origin,
            carrier: CheckedReceiverCarrier::Checked {
                place,
                view: Box::new(checked),
            },
        })
    }

    pub(super) fn produced_projection_dynamic_class(
        &self,
        exact_class: crate::identity::ClassId,
        projections: &[ObjectProjection],
    ) -> crate::identity::ClassId {
        projections.iter().fold(exact_class, |dynamic, projection| {
            let ObjectProjection::Field(field) = projection else {
                return dynamic;
            };
            let declaration = self
                .program
                .field(*field)
                .expect("resolved produced projection must reference a field");
            let ResolvedTypeKind::Class(class) = declaration.type_syntax.kind else {
                unreachable!("resolved produced object projection must have a class type")
            };
            class
        })
    }

    fn finish_shared_object_receiver(
        &self,
        pointee: super::shared_pointee::CheckedSharedPointee,
        class: crate::identity::ClassId,
        span: Span,
    ) -> CheckedObjectReceiver {
        let access = pointee.access();
        let origin = pointee.origin();
        let stable_binding = pointee.stable_binding();
        let place = HirObjectPlace {
            path: ObjectPath {
                root: stable_binding.unwrap_or(BindingId::Receiver(self.callable)),
                projections: pointee.projections().to_vec(),
                class,
                span,
            },
            access,
        };
        let carrier = match stable_binding {
            None => CheckedReceiverCarrier::View {
                view: Box::new(pointee.into_view(crate::hir::HirViewTarget::Class(class), access)),
                inspection_place: Some(Box::new(place.clone())),
            },
            Some(_) => CheckedReceiverCarrier::Place(place),
        };
        CheckedObjectReceiver { origin, carrier }
    }

    pub(in crate::typeck) fn check_object_place(
        &mut self,
        place: &ResolvedObjectPlace,
        place_use: ObjectPlaceUse,
    ) -> Option<HirObjectPlace> {
        let allow_initializing_self = !matches!(
            place_use,
            ObjectPlaceUse::Alias | ObjectPlaceUse::CopySource
        ) || !place.projections.is_empty();
        let mut checked =
            self.check_binding_place(place.root, place.span, allow_initializing_self)?;
        let mut class = checked.class();
        for &projection in &place.projections {
            match projection {
                ObjectProjection::Base(base) => {
                    assert_eq!(
                        self.program.hierarchy.direct_base(class),
                        Some(base),
                        "resolved base projection must select the direct base"
                    );
                    class = base;
                }
                ObjectProjection::Field(field) => {
                    assert_eq!(
                        field.class(),
                        class,
                        "resolved field projection must belong to its receiver class"
                    );
                    let declaration = self
                        .program
                        .field(field)
                        .expect("resolved object-place projection must reference a field");
                    let ResolvedTypeKind::Class(target) = declaration.type_syntax.kind else {
                        panic!("resolved object-place projection must have a class type");
                    };
                    class = target;
                }
            }
        }
        assert_eq!(
            class, place.class,
            "resolved object-place terminal class must match its projections"
        );
        if !matches!(place_use, ObjectPlaceUse::InitializationDestination) {
            if let Some(field) = place.direct_field() {
                if place.root == BindingId::Receiver(self.callable)
                    && !self.check_initializer_field_liveness(field, place.span)
                {
                    return None;
                }
            }
        }
        checked.path = place.clone();
        Some(checked)
    }

    pub(in crate::typeck) fn check_object_source_place(
        &mut self,
        expression: &ResolvedExpression,
    ) -> Option<HirObjectPlace> {
        if let Some(target) = self.resolved_shared_target(expression) {
            return self.reject_implicit_shared_dereference(
                expression.span(),
                target,
                "copy source must be an existing object place",
            );
        }
        let place = match expression {
            ResolvedExpression::Binding(binding)
                if matches!(self.binding_type(binding.binding), Type::Class(_)) =>
            {
                self.check_binding_place(binding.binding, binding.span, false)
            }
            ResolvedExpression::Grouped(grouped) => {
                let mut place = self.check_object_source_place(&grouped.expression)?;
                place.path.span = grouped.span;
                Some(place)
            }
            ResolvedExpression::FieldAccess(access) => {
                let field = self
                    .program
                    .field(access.field)
                    .expect("resolved copy-source field must exist");
                let ResolvedTypeKind::Class(class) = field.type_syntax.kind else {
                    self.diagnostics.push(
                        Diagnostic::error(
                            INVALID_OBJECT_CONTEXT,
                            "copy source must designate a class object",
                        )
                        .with_primary_label(access.member_span, "this field has a primitive type"),
                    );
                    return None;
                };
                if access.receiver.cast().is_some() {
                    self.diagnostics.push(
                        Diagnostic::error(
                            INVALID_OBJECT_CONTEXT,
                            "owning use of a cast field is not implemented",
                        )
                        .with_primary_label(
                            access.span,
                            "cast places are limited to direct non-owning consumers",
                        ),
                    );
                    return None;
                }
                let place = access
                    .receiver
                    .binding_path()
                    .expect("cast receiver was rejected above")
                    .clone()
                    .project_field(access.field, class, access.span);
                self.check_object_place(&place, ObjectPlaceUse::CopySource)
            }
            _ => {
                self.diagnostics.push(
                    Diagnostic::error(
                        INVALID_OBJECT_CONTEXT,
                        "copy source must be an existing object place",
                    )
                    .with_primary_label(
                        expression.span(),
                        "expected an object binding, field, or grouping",
                    ),
                );
                return None;
            }
        }?;
        Some(place)
    }

    pub(in crate::typeck) fn check_dereference_field_copy_view(
        &mut self,
        access: &crate::resolve::ResolvedFieldAccessExpr,
        field_class: crate::identity::ClassId,
    ) -> Option<HirCheckedObjectView> {
        let ResolvedObjectReceiver::Dereference {
            dereference,
            projections,
            class: receiver_class,
            ..
        } = &access.receiver
        else {
            unreachable!("shared-field copy view requires a dereference receiver")
        };
        let pointee =
            self.check_explicit_shared_pointee(dereference, projections.clone(), access.span)?;
        let view = pointee.into_view(
            crate::hir::HirViewTarget::Class(*receiver_class),
            HirAccess::ReadOnly,
        );
        Some(HirCheckedObjectView {
            view,
            consumer_target: crate::hir::HirViewTarget::Class(field_class),
            consumer_access: HirAccess::ReadOnly,
            kind: crate::hir::HirCheckedObjectViewKind::Static,
            projections: vec![ObjectProjection::Field(access.field)],
            class: Some(field_class),
            span: access.span,
        })
    }

    pub(in crate::typeck) fn check_binding_place(
        &mut self,
        binding: BindingId,
        span: Span,
        allow_initializing_self: bool,
    ) -> Option<HirObjectPlace> {
        let class = match self.binding_type(binding) {
            Type::Class(class) => class,
            _ => {
                self.diagnostics.push(
                    Diagnostic::error(
                        INVALID_ALIAS_ARGUMENT,
                        "alias argument must designate an object",
                    )
                    .with_primary_label(span, "this binding has a primitive type"),
                );
                return None;
            }
        };
        let access = self.binding_access(binding, allow_initializing_self, span)?;
        Some(HirObjectPlace {
            path: ObjectPath::root(binding, class, span),
            access,
        })
    }

    pub(in crate::typeck) fn binding_access(
        &mut self,
        binding: BindingId,
        allow_initializing_self: bool,
        span: Span,
    ) -> Option<HirAccess> {
        let access = match binding {
            BindingId::Receiver(_) => {
                let receiver = self
                    .receiver
                    .expect("resolved receiver place must occur in a member");
                if self
                    .member_body_kind
                    .is_some_and(MemberBodyKind::initializes_receiver)
                    && !allow_initializing_self
                {
                    self.diagnostics.push(
                        Diagnostic::error(
                            INVALID_ALIAS_ARGUMENT,
                            "initializer `self` is not a live alias source",
                        )
                        .with_primary_label(span, "the object becomes live after `init` returns"),
                    );
                    return None;
                }
                receiver.access
            }
            BindingId::Local(id) => {
                if self.read_only_locals.contains(&id) {
                    HirAccess::ReadOnly
                } else {
                    HirAccess::Mutable
                }
            }
            BindingId::Parameter(id) => {
                let parameter = self.parameter(id);
                lower_parameter_mode(parameter.binding_mode)
                    .required_access()
                    .unwrap_or(HirAccess::Mutable)
            }
        };
        Some(access)
    }

    pub(super) fn check_initializer_field_liveness(&mut self, field: FieldId, span: Span) -> bool {
        if !self
            .member_body_kind
            .is_some_and(MemberBodyKind::initializes_receiver)
        {
            return true;
        }
        let receiver = self
            .receiver
            .expect("an initializing member body must have a receiver");
        if field.class() != receiver.class || self.initialized_fields.contains(&field) {
            return true;
        }
        let declaration = self
            .program
            .field(field)
            .expect("selected initializer field must exist");
        self.diagnostics.push(
            Diagnostic::error(
                FIELD_INITIALIZATION,
                format!("field `{}` is used before initialization", declaration.name),
            )
            .with_primary_label(span, "this field is not initialized yet"),
        );
        false
    }

    pub(in crate::typeck) fn parameter(&self, id: ParameterId) -> &ResolvedParameter {
        self.parameters
            .get(id.index())
            .filter(|parameter| parameter.id == id)
            .expect("resolved parameter ID must exist")
    }

    pub(in crate::typeck) fn binding_type(&self, binding: BindingId) -> Type {
        assert_eq!(
            binding.callable(),
            self.callable,
            "resolved binding must belong to the current callable"
        );
        match binding {
            BindingId::Receiver(_) => Type::Class(
                self.receiver
                    .expect("receiver binding must be checked in a member body")
                    .class,
            ),
            BindingId::Parameter(id) => lower_type(self.program, &self.parameter(id).type_syntax),
            BindingId::Local(id) => lower_type(
                self.program,
                &self
                    .locals
                    .get(id.index())
                    .filter(|local| local.id == id)
                    .expect("resolved local ID must exist")
                    .type_syntax,
            ),
        }
    }
}
