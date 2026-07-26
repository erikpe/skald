//! Typed inline-object places, root capabilities, and initializer liveness.

use crate::{
    diagnostics::Diagnostic,
    hir::{
        HirAccess, HirCheckedObjectView, HirExpression, HirExpressionKind, HirFieldPlace,
        HirObjectOrigin, HirObjectPlace, Type,
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
    function::CallableChecker,
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
    pub place: HirObjectPlace,
    pub origin: HirObjectOrigin,
    pub checked_cast: Option<Box<HirCheckedObjectView>>,
    pub shared_view: Option<Box<crate::hir::HirObjectView>>,
    pub optional_view: Option<Box<crate::hir::HirObjectView>>,
    pub array_element: Option<Box<crate::hir::HirArrayElementPlace>>,
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
        if place.checked_cast.is_none()
            && place.receiver.root() == BindingId::Receiver(self.callable)
            && place.receiver.path.is_root()
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
            ty: lower_type(&field.type_syntax),
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
            receiver: checked.place,
            checked_cast: checked.checked_cast,
            shared_view: checked.shared_view,
            optional_view: checked.optional_view,
            array_element: checked.array_element,
            field,
            span,
        })
    }

    pub(in crate::typeck) fn check_object_receiver(
        &mut self,
        receiver: &ResolvedObjectReceiver,
        place_use: ObjectPlaceUse,
    ) -> Option<CheckedObjectReceiver> {
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
            let view = self.check_class_optional_view(unwrap)?;
            let access = view.access;
            let root_class = view.source.class();
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
                place,
                origin: HirObjectOrigin::Produced {
                    dynamic_class: root_class,
                    span: *span,
                },
                checked_cast: None,
                shared_view: None,
                optional_view: Some(Box::new(optional_view)),
                array_element: None,
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
            let HirExpressionKind::ArrayElement(element) = checked.kind else {
                unreachable!("array-element receiver must be an indexed projection")
            };
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
                place,
                origin,
                checked_cast: None,
                shared_view: None,
                optional_view: None,
                array_element: Some(element),
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
                place,
                origin,
                checked_cast: None,
                shared_view: None,
                optional_view: None,
                array_element: None,
            });
        };
        let mut checked = self.check_object_cast(cast)?;
        let target_class = checked
            .class
            .expect("class member receivers require a class cast target");
        checked.projections.extend_from_slice(projections);
        checked.class = Some(*class);
        // HIR member carriers still require an ordinary place alongside an
        // optional checked cast. Lowering selects the checked cast and never
        // observes this root; the resolved receiver itself has no fake binding.
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
            place,
            origin,
            checked_cast: Some(Box::new(checked)),
            shared_view: None,
            optional_view: None,
            array_element: None,
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
        let shared_view = stable_binding
            .is_none()
            .then(|| Box::new(pointee.into_view(crate::hir::HirViewTarget::Class(class), access)));
        CheckedObjectReceiver {
            place,
            origin,
            checked_cast: None,
            shared_view,
            optional_view: None,
            array_element: None,
        }
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
                if receiver.body_kind.initializes_receiver() && !allow_initializing_self {
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
            BindingId::Local(_) => HirAccess::Mutable,
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
        let Some(receiver) = self
            .receiver
            .filter(|receiver| receiver.body_kind.initializes_receiver())
        else {
            return true;
        };
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
            BindingId::Parameter(id) => lower_type(&self.parameter(id).type_syntax),
            BindingId::Local(id) => lower_type(
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
