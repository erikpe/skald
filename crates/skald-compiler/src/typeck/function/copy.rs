//! Local copy construction and live-object assignment.

use super::*;
use crate::{
    hir::{HirCopyAssignment, HirCopyConstruction, HirLocalInitializer, HirObjectPlace},
    object_path::ObjectPath,
    resolve::ResolvedObjectAssignment,
};

impl CallableChecker<'_, '_> {
    pub(super) fn check_object_local_initializer(
        &mut self,
        local: crate::identity::LocalId,
        class: ClassId,
        initializer: &crate::resolve::ResolvedExpression,
    ) -> Option<HirLocalInitializer> {
        if matches!(
            initializer,
            crate::resolve::ResolvedExpression::Construct(_)
        ) {
            return self
                .check_construction_initializer(class, initializer)
                .map(HirLocalInitializer::Construct);
        }

        let source = self.check_copy_source_place(initializer, class)?;
        let Some(operation) = self.copy_capabilities.constructor(class).selected() else {
            self.report_unavailable_copy_operation(class, true, initializer.span());
            return None;
        };
        let span = initializer.span();
        let destination_span = self
            .locals
            .get(local.index())
            .filter(|metadata| metadata.id == local)
            .expect("copy destination local must reference local metadata")
            .name_span;
        Some(HirLocalInitializer::Copy(HirCopyConstruction {
            destination: HirObjectPlace {
                path: ObjectPath::root(BindingId::Local(local), class, destination_span),
                access: HirAccess::Mutable,
            },
            source,
            operation,
            span,
        }))
    }

    pub(super) fn check_object_assignment(
        &mut self,
        assignment: &ResolvedObjectAssignment,
    ) -> CheckedStatement {
        let Some(destination) =
            self.check_object_place(&assignment.destination, ObjectPlaceUse::CopyDestination)
        else {
            return CheckedStatement::falls_through(None);
        };
        self.finish_copy_assignment(destination, &assignment.source, assignment.span)
    }

    pub(super) fn finish_copy_assignment(
        &mut self,
        destination: HirObjectPlace,
        source: &crate::resolve::ResolvedExpression,
        span: crate::source::Span,
    ) -> CheckedStatement {
        let valid_destination = match destination.root() {
            BindingId::Local(_) => true,
            BindingId::Receiver(_) if destination.projections().is_empty() => {
                self.diagnostics.push(
                    Diagnostic::error(
                        INVALID_OBJECT_CONTEXT,
                        "the complete method receiver cannot be replaced",
                    )
                    .with_primary_label(
                        destination.span(),
                        "assign one of `self`'s fields instead",
                    ),
                );
                false
            }
            BindingId::Receiver(_) => true,
            BindingId::Parameter(id) => {
                let parameter = self.parameter(id);
                if parameter.binding_mode == crate::resolve::ResolvedParameterBindingMode::Value {
                    true
                } else {
                    self.diagnostics.push(
                        Diagnostic::error(
                            INVALID_OBJECT_CONTEXT,
                            "an alias-rooted object cannot be replaced",
                        )
                        .with_primary_label(
                            destination.span(),
                            "assign an owning local, value parameter, or mutable `self` field",
                        ),
                    );
                    false
                }
            }
        };

        let mutable = destination.access == HirAccess::Mutable;
        if !mutable {
            self.diagnostics.push(
                Diagnostic::error(
                    READ_ONLY_RECEIVER,
                    "cannot assign to a read-only object place",
                )
                .with_primary_label(
                    destination.span(),
                    "object assignment requires mutable access",
                ),
            );
        }

        // Destination selection is complete before source checking, matching
        // the language's left-to-right assignment order. Stable places do not
        // require a temporary and may overlap, including exact self-assignment.
        let Some(source) = self.check_copy_source_place(source, destination.class()) else {
            return CheckedStatement::falls_through(None);
        };
        let Some(operation) = self
            .copy_capabilities
            .assignment(destination.class())
            .selected()
        else {
            self.report_unavailable_copy_operation(destination.class(), false, source.span());
            return CheckedStatement::falls_through(None);
        };

        CheckedStatement::falls_through((valid_destination && mutable).then_some(
            HirStatement::CopyAssignment(HirCopyAssignment {
                destination,
                source,
                operation,
                span,
            }),
        ))
    }
}
