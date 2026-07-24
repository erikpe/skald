//! Shared-owner compatibility, provenance, and allocation checking.

use crate::{
    diagnostics::Diagnostic,
    hir::{
        HirExpressionKind, HirSharedAllocation, HirSharedPlace, HirSharedProducer, HirSharedSource,
        HirSharedTarget, HirSharedTransfer, HirViewTarget, Type,
    },
    resolve::{
        ResolvedAllocationExpr, ResolvedConstructionMode, ResolvedExpression, ResolvedSharedTarget,
    },
};

use super::{
    expression::class_provides_view,
    function::CallableChecker,
    program::{lower_type, INVALID_SHARED_CONVERSION, UNSUPPORTED_SHARED_OPERATION},
};

pub(super) const fn lower_shared_target(target: ResolvedSharedTarget) -> HirSharedTarget {
    match target {
        ResolvedSharedTarget::Obj => HirSharedTarget::Obj,
        ResolvedSharedTarget::Class(class) => HirSharedTarget::Class(class),
        ResolvedSharedTarget::Interface(interface) => HirSharedTarget::Interface(interface),
    }
}

pub(super) fn target_accepts(
    program: &crate::resolve::ResolvedProgram,
    expected: HirSharedTarget,
    actual: HirSharedTarget,
) -> bool {
    match expected {
        HirSharedTarget::Obj => true,
        HirSharedTarget::Class(expected) => match actual {
            HirSharedTarget::Class(actual) => program
                .hierarchy
                .is_subtype(actual, expected)
                .unwrap_or(false),
            HirSharedTarget::Obj | HirSharedTarget::Interface(_) => false,
        },
        HirSharedTarget::Interface(expected) => match actual {
            HirSharedTarget::Class(actual) => {
                class_provides_view(program, actual, HirViewTarget::Interface(expected))
            }
            HirSharedTarget::Interface(actual) => actual == expected,
            HirSharedTarget::Obj => false,
        },
    }
}

impl CallableChecker<'_, '_> {
    pub(super) fn check_shared_transfer(
        &mut self,
        expression: &ResolvedExpression,
        target: HirSharedTarget,
        context: &'static str,
    ) -> Option<HirSharedTransfer> {
        let source = self.check_shared_source(expression)?;
        let actual = source.target();
        if !target_accepts(self.program, target, actual) {
            self.diagnostics.push(
                Diagnostic::error(
                    INVALID_SHARED_CONVERSION,
                    format!(
                        "{context} requires `{}` but the source has type `{}`",
                        self.shared_target_name(target),
                        self.shared_target_name(actual),
                    ),
                )
                .with_primary_label(
                    expression.span(),
                    "implicit shared conversions permit only compatible up-views",
                )
                .with_note(
                    "inline values and aliases never convert implicitly into shared ownership",
                ),
            );
            return None;
        }
        Some(HirSharedTransfer {
            operation: source.transfer(),
            source,
            target,
            span: expression.span(),
        })
    }

    fn check_shared_source(&mut self, expression: &ResolvedExpression) -> Option<HirSharedSource> {
        match expression {
            ResolvedExpression::Binding(binding) => {
                let Type::Shared(target) = self.binding_type(binding.binding) else {
                    self.report_non_shared_source(expression);
                    return None;
                };
                Some(HirSharedSource::Place(HirSharedPlace::Binding {
                    binding: binding.binding,
                    target,
                    span: binding.span,
                }))
            }
            ResolvedExpression::FieldAccess(access) => {
                let checked = self.check_field_read(access)?;
                let Type::Shared(target) = checked.ty else {
                    self.report_non_shared_source(expression);
                    return None;
                };
                let HirExpressionKind::FieldRead(place) = checked.kind else {
                    unreachable!("checked field access must remain a field read");
                };
                Some(HirSharedSource::Place(HirSharedPlace::Field {
                    place,
                    target,
                    span: checked.span,
                }))
            }
            ResolvedExpression::Allocation(allocation) => self
                .check_shared_allocation(allocation)
                .map(HirSharedProducer::Allocation)
                .map(HirSharedSource::Produced),
            ResolvedExpression::DirectCall(_)
            | ResolvedExpression::MethodCall(_)
            | ResolvedExpression::InterfaceCall(_) => {
                let call = self.check_expression(expression)?;
                if !matches!(call.ty, Type::Shared(_)) {
                    self.report_non_shared_source(expression);
                    return None;
                }
                Some(HirSharedSource::Produced(HirSharedProducer::Call(
                    Box::new(call),
                )))
            }
            ResolvedExpression::Grouped(grouped) => self.check_shared_source(&grouped.expression),
            ResolvedExpression::ObjectCast(cast)
                if matches!(
                    cast.target_mode,
                    crate::resolve::ResolvedObjectCastTargetMode::Shared { .. }
                ) =>
            {
                self.diagnostics.push(
                    Diagnostic::error(
                        UNSUPPORTED_SHARED_OPERATION,
                        "shared-owner casts are not available in typed HIR yet",
                    )
                    .with_primary_label(
                        cast.span,
                        "owner-preserving casts are implemented by a later ownership slice",
                    ),
                );
                None
            }
            ResolvedExpression::ObjectCast(cast) => {
                self.diagnostics.push(
                    Diagnostic::error(
                        INVALID_SHARED_CONVERSION,
                        "a checked place cast does not create a shared owner",
                    )
                    .with_primary_label(
                        cast.span,
                        "use `(shared T) source` to preserve an existing allocation",
                    )
                    .with_note(
                        "use `new T(copy source)` to create a distinct allocation when copy allocation becomes available",
                    ),
                );
                None
            }
            _ => {
                let _ = self.check_expression(expression);
                self.report_non_shared_source(expression);
                None
            }
        }
    }

    fn check_shared_allocation(
        &mut self,
        allocation: &ResolvedAllocationExpr,
    ) -> Option<HirSharedAllocation> {
        let ResolvedConstructionMode::Initialize { arguments } = &allocation.mode else {
            self.diagnostics.push(
                Diagnostic::error(
                    UNSUPPORTED_SHARED_OPERATION,
                    "explicit copy allocation is not available in typed HIR yet",
                )
                .with_primary_label(
                    allocation.new_span,
                    "ordinary allocation is implemented before copy allocation",
                ),
            );
            return None;
        };
        let initializer = self.select_allocation_initializer(allocation)?;
        let declaration = self
            .program
            .initializer(initializer)
            .expect("selected allocation initializer must exist");
        let arguments = self.check_arguments(
            arguments,
            &declaration.parameters,
            allocation.target_span,
            "allocation initializer",
            None,
            Some(declaration.span),
        )?;
        Some(HirSharedAllocation {
            class: allocation.class,
            initializer,
            arguments,
            span: allocation.span,
        })
    }

    fn report_non_shared_source(&mut self, expression: &ResolvedExpression) {
        let actual = self.static_expression_type_for_diagnostic(expression);
        self.diagnostics.push(
            Diagnostic::error(
                INVALID_SHARED_CONVERSION,
                "shared ownership requires an existing or produced shared owner",
            )
            .with_primary_label(
                expression.span(),
                format!("this expression has type `{}`", actual.name()),
            )
            .with_note("create a distinct shared allocation explicitly with `new`"),
        );
    }

    fn static_expression_type_for_diagnostic(&self, expression: &ResolvedExpression) -> Type {
        match expression {
            ResolvedExpression::Binding(binding) => self.binding_type(binding.binding),
            ResolvedExpression::FieldAccess(access) => self
                .program
                .field(access.field)
                .map(|field| lower_type(&field.type_syntax))
                .unwrap_or(Type::Unit),
            ResolvedExpression::Allocation(allocation) => {
                Type::Shared(HirSharedTarget::Class(allocation.class))
            }
            _ => Type::Unit,
        }
    }

    fn shared_target_name(&self, target: HirSharedTarget) -> String {
        let name = match target {
            HirSharedTarget::Obj => "Obj".to_owned(),
            HirSharedTarget::Class(class) => self
                .program
                .class(class)
                .map(|class| class.name.clone())
                .unwrap_or_else(|| class.to_string()),
            HirSharedTarget::Interface(interface) => self
                .program
                .interface(interface)
                .map(|interface| interface.name.clone())
                .unwrap_or_else(|| interface.to_string()),
        };
        format!("shared {name}")
    }
}
