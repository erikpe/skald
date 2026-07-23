//! Field initialization, construction destinations, and liveness transitions.

use super::*;
use crate::hir::{HirFieldCopyAssignment, HirFieldCopyConstruction, HirFieldPlace, HirObjectPlace};

struct FieldAssignmentTarget {
    place: HirFieldPlace,
    name: String,
    ty: Type,
    direct_self_field: bool,
    valid: bool,
}

impl CallableChecker<'_, '_> {
    pub(super) fn check_field_assignment(
        &mut self,
        assignment: &crate::resolve::ResolvedFieldAssignment,
    ) -> CheckedStatement {
        let body_kind = self
            .receiver
            .map(|receiver| receiver.body_kind)
            .unwrap_or(MemberBodyKind::MethodOrDestructor);
        let Some(target) = self.check_field_assignment_target(assignment, body_kind) else {
            return CheckedStatement::falls_through(None);
        };

        if let (Type::Class(class), MemberBodyKind::MethodOrDestructor) = (target.ty, body_kind) {
            return self.check_method_field_copy_assignment(target.place, class, assignment);
        }

        let hir = match (target.ty, body_kind) {
            (Type::Class(class), MemberBodyKind::OrdinaryInitializer) => self
                .check_field_initialization(target.place.clone(), class, &target.name, assignment),
            (Type::Class(class), MemberBodyKind::CopyConstructor) => self
                .check_copy_constructor_field_assignment(
                    target.place.clone(),
                    class,
                    &target.name,
                    assignment,
                ),
            (Type::Class(class), MemberBodyKind::CopyAssignment) => {
                self.check_field_copy_assignment(target.place.clone(), class, assignment)
            }
            (
                Type::Bool
                | Type::I64
                | Type::U64
                | Type::U8
                | Type::F64
                | Type::Unit
                | Type::Obj
                | Type::Interface(_),
                _,
            ) => self.check_primitive_field_assignment(
                target.place.clone(),
                target.ty,
                &target.name,
                assignment,
            ),
            (Type::Class(_), MemberBodyKind::MethodOrDestructor) => {
                unreachable!("method field copy assignment is handled before initializer policy")
            }
        };
        self.finish_field_assignment(target, body_kind, hir)
    }

    fn check_field_assignment_target(
        &mut self,
        assignment: &crate::resolve::ResolvedFieldAssignment,
        body_kind: MemberBodyKind,
    ) -> Option<FieldAssignmentTarget> {
        let in_initializer = body_kind.initializes_receiver();
        let place = self.check_field_place(
            &assignment.receiver,
            assignment.field,
            assignment.span,
            if in_initializer {
                ObjectPlaceUse::InitializationDestination
            } else {
                ObjectPlaceUse::Member
            },
        )?;
        let field = self
            .program
            .field(place.field)
            .expect("selected field must exist");
        let field_name = field.name.clone();
        let field_type = lower_type(&field.type_syntax);
        let mut valid = true;
        if place.receiver.access == HirAccess::ReadOnly
            && !matches!(
                (field_type, body_kind),
                (Type::Class(_), MemberBodyKind::MethodOrDestructor)
            )
        {
            self.diagnostics.push(
                Diagnostic::error(
                    READ_ONLY_RECEIVER,
                    "cannot assign through a read-only receiver",
                )
                .with_primary_label(
                    assignment.member_span,
                    "field assignment requires mutable receiver access",
                ),
            );
            valid = false;
        }
        let direct_self_field = place.receiver.root() == BindingId::Receiver(self.callable)
            && place.receiver.path.is_root();
        if in_initializer {
            if !direct_self_field {
                self.diagnostics.push(
                    Diagnostic::error(
                        INVALID_INITIALIZER_BODY,
                        "an initializer can assign only its own fields",
                    )
                    .with_primary_label(assignment.span, "expected a field of `self`"),
                );
                valid = false;
            } else if body_kind == MemberBodyKind::OrdinaryInitializer && !self.base_initialized {
                self.diagnostics.push(
                    Diagnostic::error(
                        INVALID_INITIALIZER_BODY,
                        "derived fields cannot be initialized before the base subobject",
                    )
                    .with_primary_label(
                        assignment.member_span,
                        "place `super(...)` first in the initializer",
                    ),
                );
                valid = false;
            } else if self.initialized_fields.contains(&place.field) {
                self.diagnostics.push(
                    Diagnostic::error(
                        FIELD_INITIALIZATION,
                        format!("field `{field_name}` is initialized more than once"),
                    )
                    .with_primary_label(assignment.member_span, "duplicate field initialization"),
                );
                valid = false;
            }
        }

        Some(FieldAssignmentTarget {
            place,
            name: field_name,
            ty: field_type,
            direct_self_field,
            valid,
        })
    }

    fn finish_field_assignment(
        &mut self,
        target: FieldAssignmentTarget,
        body_kind: MemberBodyKind,
        hir: Option<HirStatement>,
    ) -> CheckedStatement {
        let Some(hir) = hir else {
            return CheckedStatement::falls_through(None);
        };
        if target.valid && body_kind.initializes_receiver() && target.direct_self_field {
            self.initialized_fields.insert(target.place.field);
        }
        CheckedStatement::falls_through(target.valid.then_some(hir))
    }

    fn check_direct_field_construction(
        &mut self,
        place: HirFieldPlace,
        class: ClassId,
        field_name: &str,
        assignment: &crate::resolve::ResolvedFieldAssignment,
    ) -> Option<HirStatement> {
        self.check_field_construction(class, field_name, &assignment.value)
            .map(|construction| {
                HirStatement::FieldConstruction(HirFieldConstruction {
                    place,
                    construction,
                    span: assignment.span,
                })
            })
    }

    fn check_field_initialization(
        &mut self,
        place: HirFieldPlace,
        class: ClassId,
        field_name: &str,
        assignment: &crate::resolve::ResolvedFieldAssignment,
    ) -> Option<HirStatement> {
        if matches!(
            &assignment.value,
            crate::resolve::ResolvedExpression::Construct(construction)
                if construction.class == class
        ) {
            return self.check_direct_field_construction(place, class, field_name, assignment);
        }
        let Some(actual) = self.resolved_object_class(&assignment.value) else {
            let _ = self.check_field_construction(class, field_name, &assignment.value);
            return None;
        };
        if actual == class || self.program.hierarchy.is_subtype(actual, class) != Some(true) {
            let _ = self.check_field_construction(class, field_name, &assignment.value);
            return None;
        }
        self.check_field_copy_construction(place, class, assignment)
    }

    fn check_field_copy_construction(
        &mut self,
        place: HirFieldPlace,
        class: ClassId,
        assignment: &crate::resolve::ResolvedFieldAssignment,
    ) -> Option<HirStatement> {
        let source =
            self.check_object_source(&assignment.value, class, "field initialization source")?;
        let Some(operation) = self.copy_capabilities.constructor(class).selected() else {
            self.report_unavailable_copy_operation(class, true, assignment.value.span());
            return None;
        };
        Some(HirStatement::FieldCopyConstruction(
            HirFieldCopyConstruction {
                place,
                source,
                operation,
                span: assignment.span,
            },
        ))
    }

    fn check_copy_constructor_field_assignment(
        &mut self,
        place: HirFieldPlace,
        class: ClassId,
        field_name: &str,
        assignment: &crate::resolve::ResolvedFieldAssignment,
    ) -> Option<HirStatement> {
        if matches!(
            &assignment.value,
            crate::resolve::ResolvedExpression::Construct(construction)
                if construction.class == class
        ) {
            return self.check_direct_field_construction(place, class, field_name, assignment);
        }
        self.check_field_copy_construction(place, class, assignment)
    }

    fn check_field_copy_assignment(
        &mut self,
        place: HirFieldPlace,
        class: ClassId,
        assignment: &crate::resolve::ResolvedFieldAssignment,
    ) -> Option<HirStatement> {
        let source =
            self.check_object_source(&assignment.value, class, "field assignment source")?;
        let Some(operation) = self.copy_capabilities.assignment(class).selected() else {
            self.report_unavailable_copy_operation(class, false, assignment.value.span());
            return None;
        };
        Some(HirStatement::FieldCopyAssignment(HirFieldCopyAssignment {
            place,
            source,
            operation,
            span: assignment.span,
        }))
    }

    fn check_method_field_copy_assignment(
        &mut self,
        place: HirFieldPlace,
        class: ClassId,
        assignment: &crate::resolve::ResolvedFieldAssignment,
    ) -> CheckedStatement {
        let destination = HirObjectPlace {
            path: place
                .receiver
                .path
                .clone()
                .project_field(place.field, class, assignment.span),
            access: place.receiver.access,
        };
        self.finish_copy_assignment(destination, &assignment.value, assignment.span)
    }

    fn check_primitive_field_assignment(
        &mut self,
        place: HirFieldPlace,
        field_type: Type,
        field_name: &str,
        assignment: &crate::resolve::ResolvedFieldAssignment,
    ) -> Option<HirStatement> {
        if let crate::resolve::ResolvedExpression::Construct(construction) = &assignment.value {
            let _ = self.check_construction_arguments(construction);
            self.diagnostics.push(
                Diagnostic::error(
                    INVALID_CONSTRUCTION,
                    format!("primitive field `{field_name}` cannot contain a constructed object"),
                )
                .with_primary_label(construction.span, "expected a primitive expression"),
            );
            return None;
        }
        self.check_expression(&assignment.value).and_then(|value| {
            require_type(
                value.ty,
                field_type,
                value.span,
                "field assignment",
                self.diagnostics,
            )
            .then_some(HirStatement::FieldAssignment(HirFieldAssignment {
                place,
                value,
                span: assignment.span,
            }))
        })
    }
}
