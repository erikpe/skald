//! Typed inline-object places, root capabilities, and initializer liveness.

use crate::{
    diagnostics::Diagnostic,
    hir::{HirAccess, HirFieldPlace, HirObjectPlace, Type},
    identity::{BindingId, FieldId, ParameterId},
    object_path::ObjectPath,
    resolve::{ResolvedExpression, ResolvedObjectPlace, ResolvedParameter, ResolvedTypeKind},
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

impl CallableChecker<'_, '_> {
    pub(in crate::typeck) fn check_field_place(
        &mut self,
        place: &ResolvedObjectPlace,
        field: FieldId,
        span: Span,
        place_use: ObjectPlaceUse,
    ) -> Option<HirFieldPlace> {
        Some(HirFieldPlace {
            receiver: self.check_object_place(place, place_use)?,
            field,
            span,
        })
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
        for &field in &place.projections {
            assert_eq!(
                field.class(),
                class,
                "resolved object-place projection must belong to its receiver class"
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

    pub(super) fn check_alias_argument_place(
        &mut self,
        expression: &ResolvedExpression,
    ) -> Option<HirObjectPlace> {
        match expression {
            ResolvedExpression::Binding(binding) => {
                self.check_binding_place(binding.binding, binding.span, false)
            }
            ResolvedExpression::Grouped(grouped) => {
                let mut place = self.check_alias_argument_place(&grouped.expression)?;
                place.path.span = grouped.span;
                Some(place)
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
                    .project(access.field, class, access.span);
                self.check_object_place(&place, ObjectPlaceUse::Alias)
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

    pub(in crate::typeck) fn check_copy_source_place(
        &mut self,
        expression: &ResolvedExpression,
        expected_class: crate::identity::ClassId,
    ) -> Option<HirObjectPlace> {
        let place = match expression {
            ResolvedExpression::Binding(binding)
                if matches!(self.binding_type(binding.binding), Type::Class(_)) =>
            {
                self.check_binding_place(binding.binding, binding.span, false)
            }
            ResolvedExpression::Grouped(grouped) => {
                let mut place =
                    self.check_copy_source_place(&grouped.expression, expected_class)?;
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
                let place = access
                    .receiver
                    .clone()
                    .project(access.field, class, access.span);
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

        if place.class() != expected_class {
            let actual = &self
                .program
                .class(place.class())
                .expect("copy-source class must exist")
                .name;
            let expected = &self
                .program
                .class(expected_class)
                .expect("copy-destination class must exist")
                .name;
            self.diagnostics.push(
                Diagnostic::error(
                    INVALID_OBJECT_CONTEXT,
                    "copy source and destination must have the same class",
                )
                .with_primary_label(
                    place.span(),
                    format!("source has class `{actual}`, expected `{expected}`"),
                ),
            );
            return None;
        }

        Some(place)
    }

    fn check_binding_place(
        &mut self,
        binding: BindingId,
        span: Span,
        allow_initializing_self: bool,
    ) -> Option<HirObjectPlace> {
        let Type::Class(class) = self.binding_type(binding) else {
            self.diagnostics.push(
                Diagnostic::error(
                    INVALID_ALIAS_ARGUMENT,
                    "alias argument must designate an object",
                )
                .with_primary_label(span, "this binding has a primitive type"),
            );
            return None;
        };
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
                let Some(access) = lower_parameter_mode(parameter.binding_mode).required_access()
                else {
                    self.diagnostics.push(
                        Diagnostic::error(
                            INVALID_OBJECT_CONTEXT,
                            "an object value parameter cannot be used as an alias place",
                        )
                        .with_primary_label(span, "use an explicit alias parameter"),
                    );
                    return None;
                };
                access
            }
        };
        Some(HirObjectPlace {
            path: ObjectPath::root(binding, class, span),
            access,
        })
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

    fn parameter(&self, id: ParameterId) -> &ResolvedParameter {
        self.parameters
            .get(id.index())
            .filter(|parameter| parameter.id == id)
            .expect("resolved parameter ID must exist")
    }

    pub(super) fn binding_type(&self, binding: BindingId) -> Type {
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
