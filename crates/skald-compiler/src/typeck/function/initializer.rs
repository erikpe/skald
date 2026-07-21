//! Field initialization, construction destinations, and liveness transitions.

use super::*;
use crate::hir::HirFieldPlace;

impl CallableChecker<'_, '_> {
    pub(super) fn check_field_assignment(
        &mut self,
        assignment: &crate::resolve::ResolvedFieldAssignment,
    ) -> CheckedStatement {
        let place = self.check_field_place(
            &assignment.receiver,
            assignment.field,
            assignment.span,
            ObjectPlaceUse::InitializationDestination,
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
        if place.receiver.access == HirAccess::ReadOnly {
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
        let in_initializer = self.receiver.is_some_and(|receiver| receiver.initializer);
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
            Type::Class(class) => {
                if !in_initializer {
                    self.diagnostics.push(
                        Diagnostic::error(
                            INVALID_CONSTRUCTION,
                            "class fields can be constructed only in their owner's initializer",
                        )
                        .with_primary_label(
                            assignment.span,
                            "expected a direct uninitialized field of this initializer's `self`",
                        ),
                    );
                    valid = false;
                }
                self.check_field_construction(class, &field_name, &assignment.value)
                    .map(|construction| {
                        HirStatement::FieldConstruction(HirFieldConstruction {
                            place,
                            construction,
                            span: assignment.span,
                        })
                    })
            }
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
