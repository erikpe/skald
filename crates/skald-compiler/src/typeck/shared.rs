//! Shared-owner compatibility, provenance, and allocation checking.

use crate::{
    diagnostics::Diagnostic,
    hir::{
        HirExpressionKind, HirSharedAllocation, HirSharedCast, HirSharedCastKind, HirSharedPlace,
        HirSharedProducer, HirSharedSource, HirSharedTarget, HirSharedTransfer, HirViewTarget,
        Type,
    },
    resolve::{
        ResolvedAllocationExpr, ResolvedConstructionMode, ResolvedExpression,
        ResolvedObjectCastExpr, ResolvedSharedTarget,
    },
};

use super::{
    expression::{
        class_provides_view, classify_object_view_relation, ObjectViewRelation, ObjectViewSource,
    },
    function::CallableChecker,
    program::{
        lower_type, INVALID_OBJECT_CAST, INVALID_SHARED_CONVERSION, UNSUPPORTED_SHARED_OPERATION,
    },
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
        let source = self.check_shared_source(expression, false)?;
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

    fn check_shared_source(
        &mut self,
        expression: &ResolvedExpression,
        cast_source: bool,
    ) -> Option<HirSharedSource> {
        match expression {
            ResolvedExpression::Binding(binding) => {
                let Type::Shared(target) = self.binding_type(binding.binding) else {
                    self.report_non_shared_source(expression, cast_source);
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
                    self.report_non_shared_source(expression, cast_source);
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
                    self.report_non_shared_source(expression, cast_source);
                    return None;
                }
                Some(HirSharedSource::Produced(HirSharedProducer::Call(
                    Box::new(call),
                )))
            }
            ResolvedExpression::Grouped(grouped) => {
                self.check_shared_source(&grouped.expression, cast_source)
            }
            ResolvedExpression::ObjectCast(cast)
                if matches!(
                    cast.target_mode,
                    crate::resolve::ResolvedObjectCastTargetMode::Shared { .. }
                ) =>
            {
                self.check_shared_cast(cast)
                    .map(Box::new)
                    .map(HirSharedProducer::Cast)
                    .map(HirSharedSource::Produced)
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
                self.report_non_shared_source(expression, cast_source);
                None
            }
        }
    }

    fn check_shared_cast(&mut self, cast: &ResolvedObjectCastExpr) -> Option<HirSharedCast> {
        let source = self.check_shared_source(&cast.source, true)?;
        let target_view =
            self.check_view_target(&cast.target, cast.target_span, INVALID_OBJECT_CAST)?;
        let target = shared_target_from_view(target_view);
        let exact_dynamic_class = shared_source_exact_dynamic_class(&source);
        let relation_source = exact_dynamic_class.map_or_else(
            || ObjectViewSource::Dynamic(shared_target_view(source.target())),
            ObjectViewSource::ExactClass,
        );
        let kind = match classify_object_view_relation(self.program, relation_source, target_view) {
            ObjectViewRelation::StaticSuccess => HirSharedCastKind::Static,
            ObjectViewRelation::Runtime => HirSharedCastKind::RuntimeTerminate,
            ObjectViewRelation::StaticFailure => {
                self.diagnostics.push(
                    Diagnostic::error(INVALID_OBJECT_CAST, "shared-owner cast can never succeed")
                        .with_primary_label(
                            cast.target_span,
                            "no possible dynamic class provides this shared view",
                        )
                        .with_secondary_label(source.span(), "shared source"),
                );
                return None;
            }
        };
        Some(HirSharedCast {
            source,
            target,
            kind,
            exact_dynamic_class,
            span: cast.span,
        })
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

    fn report_non_shared_source(&mut self, expression: &ResolvedExpression, cast_source: bool) {
        let actual = self.static_expression_type_for_diagnostic(expression);
        self.diagnostics.push(
            Diagnostic::error(
                if cast_source {
                    INVALID_OBJECT_CAST
                } else {
                    INVALID_SHARED_CONVERSION
                },
                if cast_source {
                    "shared-owner cast requires an existing or produced shared owner"
                } else {
                    "shared ownership requires an existing or produced shared owner"
                },
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

const fn shared_target_from_view(target: HirViewTarget) -> HirSharedTarget {
    match target {
        HirViewTarget::Obj => HirSharedTarget::Obj,
        HirViewTarget::Class(class) => HirSharedTarget::Class(class),
        HirViewTarget::Interface(interface) => HirSharedTarget::Interface(interface),
    }
}

const fn shared_target_view(target: HirSharedTarget) -> HirViewTarget {
    match target {
        HirSharedTarget::Obj => HirViewTarget::Obj,
        HirSharedTarget::Class(class) => HirViewTarget::Class(class),
        HirSharedTarget::Interface(interface) => HirViewTarget::Interface(interface),
    }
}

fn shared_source_exact_dynamic_class(source: &HirSharedSource) -> Option<crate::identity::ClassId> {
    match source {
        HirSharedSource::Produced(HirSharedProducer::Allocation(allocation)) => {
            Some(allocation.class)
        }
        HirSharedSource::Produced(HirSharedProducer::Cast(cast)) => cast.exact_dynamic_class,
        HirSharedSource::Place(_) | HirSharedSource::Produced(HirSharedProducer::Call(_)) => None,
    }
}
