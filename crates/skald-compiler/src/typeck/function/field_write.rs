//! Central instance-field replacement policy.

use super::*;
use crate::{
    hir::{HirCopyCapability, HirFieldWriteAuthorization},
    identity::CallableId,
    source::Span,
};

pub(super) enum FieldWriteDecision {
    ConstructionInitialization,
    Authorized(HirFieldWriteAuthorization),
    DeferredObjectAssignment,
    Rejected,
}

pub(super) struct FieldWriteContext {
    pub(super) access: HirAccess,
    pub(super) has_inspection_place: bool,
    pub(super) direct_self_field: bool,
    pub(super) field: FieldId,
    pub(super) field_type: Type,
    pub(super) cell_span: Option<Span>,
    pub(super) final_span: Option<Span>,
    pub(super) member_span: Span,
    pub(super) body_kind: MemberBodyKind,
}

impl CallableChecker<'_, '_> {
    pub(super) fn decide_field_write(&mut self, context: FieldWriteContext) -> FieldWriteDecision {
        if context.body_kind.initializes_receiver()
            && context.direct_self_field
            && self.class_owner == Some(context.field.class())
        {
            return FieldWriteDecision::ConstructionInitialization;
        }

        if let Some(final_span) = context.final_span {
            if let CallableId::CopyAssignment(operation) = self.callable {
                if context.body_kind == MemberBodyKind::CopyAssignment
                    && context.direct_self_field
                    && self.class_owner == Some(context.field.class())
                    && matches!(
                        self.copy_capabilities.assignment(context.field.class()),
                        HirCopyCapability::User(copy) if copy.operation == operation
                    )
                {
                    return FieldWriteDecision::Authorized(
                        HirFieldWriteAuthorization::DeclaringClassFinalAssignment(operation),
                    );
                }
            }
            let field = self
                .program
                .field(context.field)
                .expect("selected final field must exist");
            self.diagnostics.push(
                Diagnostic::error(
                    super::super::program::FINAL_FIELD_REPLACEMENT,
                    format!("final field `{}` cannot be replaced", field.name),
                )
                .with_primary_label(
                    context.member_span,
                    "final fields can be initialized during construction but not replaced directly",
                )
                .with_secondary_label(final_span, "field declared final here"),
            );
            return FieldWriteDecision::Rejected;
        }

        if context.access == HirAccess::Mutable {
            return FieldWriteDecision::Authorized(HirFieldWriteAuthorization::Mutable);
        }
        if context.cell_span.is_some() && self.class_owner == Some(context.field.class()) {
            return FieldWriteDecision::Authorized(HirFieldWriteAuthorization::DeclaringClassCell);
        }
        if context.cell_span.is_none()
            && matches!(context.field_type, Type::Class(_))
            && context.has_inspection_place
        {
            return FieldWriteDecision::DeferredObjectAssignment;
        }

        self.diagnostics.push(
            Diagnostic::error(
                super::super::program::READ_ONLY_RECEIVER,
                "cannot assign through a read-only receiver",
            )
            .with_primary_label(
                context.member_span,
                "field assignment requires mutable receiver access",
            ),
        );
        FieldWriteDecision::Rejected
    }
}
