//! Shared-owner compatibility, provenance, and allocation checking.

use crate::{
    diagnostics::Diagnostic,
    hir::{
        HirConstructionMode, HirExpressionKind, HirSharedAllocation, HirSharedAllocationMode,
        HirSharedCast, HirSharedCastKind, HirSharedPlace, HirSharedProducer, HirSharedSource,
        HirSharedTarget, HirSharedTransfer, HirViewTarget, Type,
    },
    resolve::{
        ResolvedAllocationExpr, ResolvedConstructionMode, ResolvedExpression,
        ResolvedObjectCastExpr, ResolvedSharedTarget,
    },
    source::Span,
};

use super::{
    expression::{
        class_provides_view, classify_object_view_relation, ObjectViewRelation, ObjectViewSource,
    },
    function::CallableChecker,
    program::{
        lower_type, IMPLICIT_SHARED_DEREFERENCE, INVALID_OBJECT_CAST, INVALID_SHARED_CONVERSION,
    },
};

pub(super) const fn lower_shared_target(target: ResolvedSharedTarget) -> HirSharedTarget {
    match target {
        ResolvedSharedTarget::Obj => HirSharedTarget::Obj,
        ResolvedSharedTarget::Class(class) => HirSharedTarget::Class(class),
        ResolvedSharedTarget::Interface(interface) => HirSharedTarget::Interface(interface),
        ResolvedSharedTarget::Array(array) => HirSharedTarget::Array(array),
    }
}

pub(super) fn target_accepts(
    program: &crate::resolve::ResolvedProgram,
    expected: HirSharedTarget,
    actual: HirSharedTarget,
) -> bool {
    match expected {
        HirSharedTarget::Obj => !matches!(actual, HirSharedTarget::Array(_)),
        HirSharedTarget::Class(expected) => match actual {
            HirSharedTarget::Class(actual) => program
                .hierarchy
                .is_subtype(actual, expected)
                .unwrap_or(false),
            HirSharedTarget::Obj | HirSharedTarget::Interface(_) | HirSharedTarget::Array(_) => {
                false
            }
        },
        HirSharedTarget::Interface(expected) => match actual {
            HirSharedTarget::Class(actual) => {
                class_provides_view(program, actual, HirViewTarget::Interface(expected))
            }
            HirSharedTarget::Interface(actual) => actual == expected,
            HirSharedTarget::Obj | HirSharedTarget::Array(_) => false,
        },
        HirSharedTarget::Array(expected) => {
            matches!(actual, HirSharedTarget::Array(actual) if actual == expected)
        }
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

    pub(super) fn check_shared_source(
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
            ResolvedExpression::StaticFieldAccess(access) => {
                let (place, ty) = self.check_static_place(access.field, access.span)?;
                let Type::Shared(target) = ty else {
                    self.report_non_shared_source(expression, cast_source);
                    return None;
                };
                Some(HirSharedSource::Place(HirSharedPlace::Static {
                    place,
                    target,
                    span: access.span,
                }))
            }
            ResolvedExpression::ArrayProjection(_) => {
                let checked = self.check_expression(expression)?;
                let Type::Shared(target) = checked.ty else {
                    self.report_non_shared_source(expression, cast_source);
                    return None;
                };
                let HirExpressionKind::ArrayElement(place) = checked.kind else {
                    self.report_non_shared_source(expression, cast_source);
                    return None;
                };
                Some(HirSharedSource::Place(HirSharedPlace::ArrayElement {
                    place,
                    target,
                    span: checked.span,
                }))
            }
            ResolvedExpression::Allocation(allocation) => self
                .check_shared_allocation(allocation)
                .map(HirSharedProducer::Allocation)
                .map(HirSharedSource::Produced),
            ResolvedExpression::ArrayConstruction(construction) => {
                let checked = self.check_array_construction(construction)?;
                let HirExpressionKind::ArrayConstruction(construction) = checked.kind else {
                    unreachable!("checked array construction must retain its typed node")
                };
                if construction.ownership != crate::hir::HirArrayOwnership::Shared {
                    self.report_non_shared_source(expression, cast_source);
                    return None;
                }
                Some(HirSharedSource::Produced(
                    HirSharedProducer::ArrayAllocation(construction),
                ))
            }
            ResolvedExpression::DirectCall(_)
            | ResolvedExpression::StaticCall(_)
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
                        "use `new T(copy source)` to create a distinct exact-class allocation",
                    ),
                );
                None
            }
            ResolvedExpression::Unwrap(unwrap) => {
                let operand =
                    self.require_optional_operand(&unwrap.source, unwrap.span, "checked unwrap")?;
                if !matches!(
                    operand,
                    crate::hir::HirOptionalOperand::SharedPlace(_)
                        | crate::hir::HirOptionalOperand::SharedProduced(_)
                ) {
                    self.report_non_shared_source(expression, cast_source);
                    return None;
                }
                Some(HirSharedSource::Produced(
                    HirSharedProducer::OptionalUnwrap(operand),
                ))
            }
            ResolvedExpression::Dereference(dereference) => {
                self.diagnostics.push(
                    Diagnostic::error(
                        if cast_source {
                            INVALID_OBJECT_CAST
                        } else {
                            INVALID_SHARED_CONVERSION
                        },
                        "a dereferenced pointee is not a shared owner",
                    )
                    .with_primary_label(
                        dereference.span,
                        "remove `*` when this context requires the owner handle",
                    )
                    .with_secondary_label(
                        dereference.operator_span,
                        "dereference selects a bounded non-owning place",
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
        let exact_dynamic_class = source.exact_dynamic_class();
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
        let mode = match &allocation.mode {
            ResolvedConstructionMode::Initialize { arguments } => {
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
                HirSharedAllocationMode::Initialize {
                    initializer,
                    arguments,
                }
            }
            ResolvedConstructionMode::Copy { copy_span, source } => {
                let HirConstructionMode::Copy { source, operation } = self
                    .check_copy_construction_mode(
                        allocation.class,
                        source,
                        allocation.target_span,
                        allocation.span,
                        *copy_span,
                        "copy allocation",
                    )?
                else {
                    unreachable!("explicit copy mode must remain distinct from initialization")
                };
                HirSharedAllocationMode::Copy { source, operation }
            }
        };
        Some(HirSharedAllocation {
            class: allocation.class,
            mode,
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

    pub(in crate::typeck) fn shared_target_name(&self, target: HirSharedTarget) -> String {
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
            HirSharedTarget::Array(array) => format!("array {array}"),
        };
        format!("shared {name}")
    }

    pub(in crate::typeck) fn reject_implicit_shared_dereference<T>(
        &mut self,
        span: Span,
        target: HirSharedTarget,
        consumer_requirement: &str,
    ) -> Option<T> {
        let diagnostic = self
            .implicit_shared_dereference_diagnostic(span, target)
            .with_note(consumer_requirement);
        self.diagnostics.push(diagnostic);
        None
    }

    pub(in crate::typeck) fn implicit_shared_dereference_diagnostic(
        &self,
        span: Span,
        target: HirSharedTarget,
    ) -> Diagnostic {
        Diagnostic::error(
            IMPLICIT_SHARED_DEREFERENCE,
            "shared owner must be explicitly dereferenced for object-place use",
        )
        .with_primary_label(
            span,
            format!(
                "this expression has type `{}`; use `*` to select its pointee",
                self.shared_target_name(target)
            ),
        )
    }

    pub(super) fn resolved_shared_target(
        &self,
        expression: &ResolvedExpression,
    ) -> Option<HirSharedTarget> {
        match expression {
            ResolvedExpression::Binding(binding) => match self.binding_type(binding.binding) {
                Type::Shared(target) => Some(target),
                _ => None,
            },
            ResolvedExpression::FieldAccess(access) => {
                self.program
                    .field(access.field)
                    .and_then(|field| match field.type_syntax.kind {
                        crate::resolve::ResolvedTypeKind::Shared(target) => {
                            Some(lower_shared_target(target))
                        }
                        _ => None,
                    })
            }
            ResolvedExpression::Allocation(allocation) => {
                Some(HirSharedTarget::Class(allocation.class))
            }
            ResolvedExpression::DirectCall(call) => self
                .program
                .declarations
                .get(call.function)
                .and_then(|declaration| match declaration.return_type.kind {
                    crate::resolve::ResolvedTypeKind::Shared(target) => {
                        Some(lower_shared_target(target))
                    }
                    _ => None,
                }),
            ResolvedExpression::StaticCall(call) => {
                self.program
                    .method(call.method)
                    .and_then(|method| match method.return_type.kind {
                        crate::resolve::ResolvedTypeKind::Shared(target) => {
                            Some(lower_shared_target(target))
                        }
                        _ => None,
                    })
            }
            ResolvedExpression::MethodCall(call) => {
                self.program
                    .method(call.method)
                    .and_then(|method| match method.return_type.kind {
                        crate::resolve::ResolvedTypeKind::Shared(target) => {
                            Some(lower_shared_target(target))
                        }
                        _ => None,
                    })
            }
            ResolvedExpression::InterfaceCall(call) => self
                .program
                .interface(call.interface)
                .and_then(|interface| interface.requirements.get(call.requirement.index()))
                .and_then(|requirement| match requirement.return_type.kind {
                    crate::resolve::ResolvedTypeKind::Shared(target) => {
                        Some(lower_shared_target(target))
                    }
                    _ => None,
                }),
            ResolvedExpression::Grouped(grouped) => {
                self.resolved_shared_target(&grouped.expression)
            }
            ResolvedExpression::ObjectCast(cast)
                if matches!(
                    cast.target_mode,
                    crate::resolve::ResolvedObjectCastTargetMode::Shared { .. }
                ) =>
            {
                match cast.target.kind {
                    crate::resolve::ResolvedTypeKind::Class(class) => {
                        Some(HirSharedTarget::Class(class))
                    }
                    crate::resolve::ResolvedTypeKind::Interface(interface) => {
                        Some(HirSharedTarget::Interface(interface))
                    }
                    crate::resolve::ResolvedTypeKind::Obj => Some(HirSharedTarget::Obj),
                    _ => None,
                }
            }
            ResolvedExpression::Unwrap(unwrap) => {
                self.resolved_optional_shared_target(&unwrap.source)
            }
            _ => None,
        }
    }

    fn resolved_optional_shared_target(
        &self,
        expression: &ResolvedExpression,
    ) -> Option<HirSharedTarget> {
        let resolved = match expression {
            ResolvedExpression::Binding(binding) => {
                return match self.binding_type(binding.binding) {
                    Type::OptionalShared(target) => Some(target),
                    _ => None,
                }
            }
            ResolvedExpression::FieldAccess(access) => {
                self.program.field(access.field)?.type_syntax.kind
            }
            ResolvedExpression::DirectCall(call) => {
                self.program
                    .declarations
                    .get(call.function)?
                    .return_type
                    .kind
            }
            ResolvedExpression::StaticCall(call) => {
                self.program.method(call.method)?.return_type.kind
            }
            ResolvedExpression::MethodCall(call) => {
                self.program.method(call.method)?.return_type.kind
            }
            ResolvedExpression::InterfaceCall(call) => {
                self.program
                    .interface(call.interface)?
                    .requirements
                    .get(call.requirement.index())?
                    .return_type
                    .kind
            }
            ResolvedExpression::Grouped(grouped) => {
                return self.resolved_optional_shared_target(&grouped.expression)
            }
            _ => return None,
        };
        match resolved {
            crate::resolve::ResolvedTypeKind::OptionalShared { target, .. } => {
                Some(lower_shared_target(target))
            }
            _ => None,
        }
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
        HirSharedTarget::Array(_) => {
            panic!("array pointee views are typed by the array projection checker")
        }
    }
}
