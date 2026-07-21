//! Field initialization, construction destinations, and liveness transitions.

use super::*;
use crate::hir::{HirFieldCopyAssignment, HirFieldCopyConstruction, HirFieldPlace, HirObjectPlace};

impl CallableChecker<'_, '_> {
    pub(super) fn check_field_assignment(
        &mut self,
        assignment: &crate::resolve::ResolvedFieldAssignment,
    ) -> CheckedStatement {
        let body_kind = self
            .receiver
            .map(|receiver| receiver.body_kind)
            .unwrap_or(MemberBodyKind::MethodOrDestructor);
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
        );
        let Some(place) = place else {
            return CheckedStatement::falls_through(None);
        };
        let field = self
            .program
            .field(place.field)
            .expect("selected field must exist");
        let field_name = field.name.clone();
        let field_type = lower_type(&field.type_syntax);
        let field_id = place.field;
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

        let hir = match field_type {
            Type::Class(class) => match body_kind {
                MemberBodyKind::OrdinaryInitializer => self
                    .check_field_construction(class, &field_name, &assignment.value)
                    .map(|construction| {
                        HirStatement::FieldConstruction(HirFieldConstruction {
                            place,
                            construction,
                            span: assignment.span,
                        })
                    }),
                MemberBodyKind::CopyConstructor => {
                    if matches!(
                        &assignment.value,
                        crate::resolve::ResolvedExpression::Construct(_)
                    ) {
                        self.check_field_construction(class, &field_name, &assignment.value)
                            .map(|construction| {
                                HirStatement::FieldConstruction(HirFieldConstruction {
                                    place,
                                    construction,
                                    span: assignment.span,
                                })
                            })
                    } else {
                        let Some(source) = self.check_copy_source_place(&assignment.value, class)
                        else {
                            return CheckedStatement::falls_through(None);
                        };
                        let operation = self.copy_capabilities.constructor(class).selected();
                        let Some(operation) = operation else {
                            self.report_unavailable_copy_operation(
                                class,
                                true,
                                assignment.value.span(),
                            );
                            return CheckedStatement::falls_through(None);
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
                }
                MemberBodyKind::CopyAssignment => {
                    let Some(source) = self.check_copy_source_place(&assignment.value, class)
                    else {
                        return CheckedStatement::falls_through(None);
                    };
                    let operation = self.copy_capabilities.assignment(class).selected();
                    let Some(operation) = operation else {
                        self.report_unavailable_copy_operation(
                            class,
                            false,
                            assignment.value.span(),
                        );
                        return CheckedStatement::falls_through(None);
                    };
                    Some(HirStatement::FieldCopyAssignment(HirFieldCopyAssignment {
                        place,
                        source,
                        operation,
                        span: assignment.span,
                    }))
                }
                MemberBodyKind::MethodOrDestructor => {
                    let destination = HirObjectPlace {
                        path: place.receiver.path.clone().project(
                            place.field,
                            class,
                            assignment.span,
                        ),
                        access: place.receiver.access,
                    };
                    return self.finish_copy_assignment(
                        destination,
                        &assignment.value,
                        assignment.span,
                    );
                }
            },
            _ => self.check_primitive_field_value(place, field_type, &field_name, assignment),
        };
        let Some(hir) = hir else {
            return CheckedStatement::falls_through(None);
        };
        if valid && in_initializer && direct_self_field {
            self.initialized_fields.insert(field_id);
        }
        CheckedStatement::falls_through(valid.then_some(hir))
    }

    pub(super) fn report_unavailable_copy_operation(
        &mut self,
        class: crate::identity::ClassId,
        construction: bool,
        span: crate::source::Span,
    ) {
        let class_name = &self
            .program
            .class(class)
            .expect("copy capability class must exist")
            .name;
        let operation = if construction {
            "copy construction"
        } else {
            "copy assignment"
        };
        let failure = if construction {
            self.copy_capabilities.constructor_failure(class)
        } else {
            self.copy_capabilities.assignment_failure(class)
        };
        let mut diagnostic = Diagnostic::error(
            COPY_OPERATION_UNAVAILABLE,
            format!("class `{class_name}` does not support {operation}"),
        )
        .with_primary_label(span, format!("{operation} is required here"));
        if let Some(path) = failure.filter(|path| !path.is_empty()) {
            let names = path
                .iter()
                .map(|field| {
                    let declaration = self
                        .program
                        .field(*field)
                        .expect("capability failure field must exist");
                    let owner = self
                        .program
                        .class(field.class())
                        .expect("capability failure owner must exist");
                    format!("{}.{}", owner.name, declaration.name)
                })
                .collect::<Vec<_>>()
                .join(" -> ");
            diagnostic = diagnostic.with_note(format!("first unavailable field path: {names}"));
        }
        self.diagnostics.push(diagnostic);
    }

    fn check_primitive_field_value(
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
