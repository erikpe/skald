//! Local copy construction and live-object assignment.

use super::*;
use crate::{
    hir::{
        HirCopyAssignment, HirCopyConstruction, HirExpression, HirExpressionKind,
        HirLocalInitializer, HirObjectCall, HirObjectCallTarget, HirObjectInitialization,
        HirObjectPlace, HirObjectProducer, HirObjectSource,
    },
    object_path::ObjectPath,
    resolve::ResolvedObjectAssignment,
};

impl CallableChecker<'_, '_> {
    pub(in crate::typeck) fn report_unavailable_copy_operation(
        &mut self,
        class: ClassId,
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

    pub(super) fn check_object_local_initializer(
        &mut self,
        local: crate::identity::LocalId,
        class: ClassId,
        initializer: &crate::resolve::ResolvedExpression,
    ) -> Option<HirLocalInitializer> {
        let destination = self.object_local_destination(local, class);
        if matches!(
            initializer,
            crate::resolve::ResolvedExpression::Construct(_)
        ) {
            let construction = self.check_construction_initializer(class, initializer)?;
            // Elision does not change validity: the corresponding non-elided
            // execution must still have a selected copy constructor.
            let Some(elided_copy) = self.copy_capabilities.constructor(class).selected() else {
                self.report_unavailable_copy_operation(class, true, initializer.span());
                return None;
            };
            return Some(HirLocalInitializer::Object(HirObjectInitialization {
                destination,
                span: construction.span,
                producer: HirObjectProducer::Construct(construction),
                elided_copy: Some(elided_copy),
            }));
        }

        if is_object_call_source(initializer) {
            let expression = self.check_expression(initializer)?;
            if !require_type(
                expression.ty,
                Type::Class(class),
                expression.span,
                "object result initializer",
                self.diagnostics,
            ) {
                return None;
            }
            let call = lower_object_call(expression, class);
            return Some(HirLocalInitializer::Object(HirObjectInitialization {
                destination,
                span: call.span,
                producer: HirObjectProducer::Call(call),
                elided_copy: None,
            }));
        }

        let source = self.check_object_source(initializer, class, "object initializer")?;
        let Some(operation) = self.copy_capabilities.constructor(class).selected() else {
            self.report_unavailable_copy_operation(class, true, initializer.span());
            return None;
        };
        let span = initializer.span();
        Some(HirLocalInitializer::Copy(HirCopyConstruction {
            destination,
            source,
            operation,
            span,
        }))
    }

    pub(crate) fn check_object_source(
        &mut self,
        expression: &crate::resolve::ResolvedExpression,
        class: ClassId,
        context: &'static str,
    ) -> Option<HirObjectSource> {
        if let Some(construction) = construction_through_groups(expression) {
            let mut construction =
                self.check_object_construction(class, construction, "object destination")?;
            construction.span = expression.span();
            return Some(HirObjectSource::Produced(HirObjectProducer::Construct(
                construction,
            )));
        }
        if is_object_call_source(expression) {
            let checked = self.check_expression(expression)?;
            if !require_type(
                checked.ty,
                Type::Class(class),
                checked.span,
                context,
                self.diagnostics,
            ) {
                return None;
            }
            return Some(HirObjectSource::Produced(HirObjectProducer::Call(
                lower_object_call(checked, class),
            )));
        }
        self.check_copy_source_place(expression, class)
            .map(HirObjectSource::Place)
    }

    fn object_local_destination(
        &self,
        local: crate::identity::LocalId,
        class: ClassId,
    ) -> HirObjectPlace {
        let destination_span = self
            .locals
            .get(local.index())
            .filter(|metadata| metadata.id == local)
            .expect("object destination local must reference local metadata")
            .name_span;
        HirObjectPlace {
            path: ObjectPath::root(BindingId::Local(local), class, destination_span),
            access: HirAccess::Mutable,
        }
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
        let Some(source) =
            self.check_object_source(source, destination.class(), "object assignment source")
        else {
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

fn is_object_call_source(expression: &crate::resolve::ResolvedExpression) -> bool {
    match expression {
        crate::resolve::ResolvedExpression::DirectCall(_)
        | crate::resolve::ResolvedExpression::MethodCall(_) => true,
        crate::resolve::ResolvedExpression::Grouped(grouped) => {
            is_object_call_source(&grouped.expression)
        }
        _ => false,
    }
}

fn construction_through_groups(
    expression: &crate::resolve::ResolvedExpression,
) -> Option<&crate::resolve::ResolvedConstructExpr> {
    match expression {
        crate::resolve::ResolvedExpression::Construct(construction) => Some(construction),
        crate::resolve::ResolvedExpression::Grouped(grouped) => {
            construction_through_groups(&grouped.expression)
        }
        _ => None,
    }
}

fn lower_object_call(expression: HirExpression, class: ClassId) -> HirObjectCall {
    let span = expression.span;
    match expression.kind {
        HirExpressionKind::DirectCall {
            function,
            arguments,
        } => HirObjectCall {
            target: HirObjectCallTarget::Direct(function),
            arguments,
            class,
            span,
        },
        HirExpressionKind::MethodCall {
            receiver,
            method,
            arguments,
        } => HirObjectCall {
            target: HirObjectCallTarget::Method { receiver, method },
            arguments,
            class,
            span,
        },
        HirExpressionKind::Grouped(inner) => {
            let mut call = lower_object_call(*inner, class);
            call.span = span;
            call
        }
        _ => unreachable!("syntactic object call must type-check as a call expression"),
    }
}
