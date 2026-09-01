//! Direct object construction and initializer argument checking.

use super::*;
use crate::source::Span;

impl CallableChecker<'_, '_> {
    pub(super) fn check_construction_initializer(
        &mut self,
        expected_class: ClassId,
        expression: &crate::resolve::ResolvedExpression,
    ) -> Option<HirConstruction> {
        let crate::resolve::ResolvedExpression::Construct(construction) = expression else {
            self.diagnostics.push(
                Diagnostic::error(
                    INVALID_OBJECT_CONTEXT,
                    "an object local must be initialized by direct construction",
                )
                .with_primary_label(
                    expression.span(),
                    "expected an ungrouped `Class(...)` expression",
                ),
            );
            return None;
        };
        self.check_object_construction(expected_class, construction, "object local")
    }

    pub(super) fn check_object_construction(
        &mut self,
        expected_class: ClassId,
        construction: &crate::resolve::ResolvedConstructExpr,
        destination: &str,
    ) -> Option<HirConstruction> {
        if construction.class != expected_class {
            let actual_name = &self
                .program
                .class(construction.class)
                .expect("resolved constructor class must exist")
                .name;
            let expected_name = &self
                .program
                .class(expected_class)
                .expect("resolved local class must exist")
                .name;
            self.diagnostics.push(
                Diagnostic::error(
                    INVALID_CONSTRUCTION,
                    format!("constructor type does not match the {destination}"),
                )
                .with_primary_label(
                    construction.callee_span,
                    format!("constructs `{actual_name}`"),
                )
                .with_note(format!("the {destination} requires `{expected_name}`")),
            );
            return None;
        }
        self.check_construction_arguments(construction)
    }

    pub(super) fn check_field_construction(
        &mut self,
        expected_class: ClassId,
        field_name: &str,
        expression: &crate::resolve::ResolvedExpression,
    ) -> Option<HirConstruction> {
        let crate::resolve::ResolvedExpression::Construct(construction) = expression else {
            let expected_name = &self
                .program
                .class(expected_class)
                .expect("resolved field class must exist")
                .name;
            self.diagnostics.push(
                Diagnostic::error(
                    INVALID_CONSTRUCTION,
                    format!("class field `{field_name}` requires direct construction"),
                )
                .with_primary_label(
                    expression.span(),
                    format!("expected an ungrouped `{expected_name}(...)` construction"),
                ),
            );
            return None;
        };
        if construction.class != expected_class {
            let actual_name = &self
                .program
                .class(construction.class)
                .expect("resolved constructor class must exist")
                .name;
            let expected_name = &self
                .program
                .class(expected_class)
                .expect("resolved field class must exist")
                .name;
            self.diagnostics.push(
                Diagnostic::error(
                    INVALID_CONSTRUCTION,
                    format!("constructor type does not match class field `{field_name}`"),
                )
                .with_primary_label(
                    construction.callee_span,
                    format!("constructs `{actual_name}`"),
                )
                .with_note(format!("the field requires `{expected_name}`")),
            );
            return None;
        }
        self.check_construction_arguments(construction)
    }

    pub(super) fn check_construction_arguments(
        &mut self,
        construction: &crate::resolve::ResolvedConstructExpr,
    ) -> Option<HirConstruction> {
        let mode = match &construction.mode {
            crate::resolve::ResolvedConstructionMode::Initialize { arguments } => {
                let initializer_id = self.select_construction_initializer(construction)?;
                let initializer = self
                    .program
                    .initializer(initializer_id)
                    .expect("selected construction must reference an initializer");
                let arguments = self.check_arguments(
                    arguments,
                    &initializer.parameters,
                    construction.callee_span,
                    "initializer",
                    None,
                    None,
                )?;
                crate::hir::HirConstructionMode::Initialize {
                    initializer: initializer_id,
                    arguments,
                }
            }
            crate::resolve::ResolvedConstructionMode::Copy { copy_span, source } => self
                .check_copy_construction_mode(
                    construction.class,
                    source,
                    construction.callee_span,
                    construction.span,
                    *copy_span,
                    "copy construction",
                )?,
        };
        Some(HirConstruction {
            class: construction.class,
            mode,
            span: construction.span,
        })
    }

    pub(in crate::typeck) fn check_copy_construction_mode(
        &mut self,
        class: ClassId,
        source: &crate::resolve::ResolvedExpression,
        target_span: Span,
        construction_span: Span,
        copy_span: Span,
        context: &'static str,
    ) -> Option<crate::hir::HirConstructionMode> {
        let source = if crate::typeck::function::copy::is_checked_object_source_expression(source) {
            self.check_object_source(source, class, context)?
        } else {
            let checked =
                self.check_copy_construction_view(source, class, target_span, construction_span)?;
            crate::hir::HirObjectSource::Checked(Box::new(checked))
        };
        let Some(operation) = self.copy_capabilities.constructor(class).selected() else {
            self.report_unavailable_copy_operation(class, true, copy_span);
            return None;
        };
        Some(crate::hir::HirConstructionMode::Copy {
            source: Box::new(source),
            operation,
        })
    }
}
