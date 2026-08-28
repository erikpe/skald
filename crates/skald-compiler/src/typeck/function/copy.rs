//! Local copy construction and live-object assignment.

use super::*;
use crate::{
    hir::{
        HirCopyAssignment, HirCopyConstruction, HirExpression, HirExpressionKind,
        HirLocalInitializer, HirObjectCall, HirObjectCallTarget, HirObjectInitialization,
        HirObjectPlace, HirObjectProducer, HirObjectSlice, HirObjectSource,
    },
    object_path::ObjectPath,
    resolve::ResolvedObjectAssignment,
    typeck::capabilities::CopyPathElement,
};

impl CallableChecker<'_, '_> {
    pub(in crate::typeck) fn check_object_destination_initialization(
        &mut self,
        class: ClassId,
        initializer: &crate::resolve::ResolvedExpression,
        context: &'static str,
    ) -> Option<crate::hir::HirObjectDestinationInitialization> {
        if let crate::resolve::ResolvedExpression::Construct(construction) = initializer {
            if construction.class == class {
                let construction = self.check_object_construction(class, construction, context)?;
                let span = construction.span;
                return Some(crate::hir::HirObjectDestinationInitialization::Direct {
                    producer: HirObjectProducer::Construct(construction),
                    span,
                });
            }
        }

        if is_ungrouped_object_call(initializer)
            && self.resolved_object_class(initializer) == Some(class)
        {
            let expression = self.check_expression(initializer)?;
            if !require_type(
                expression.ty,
                Type::Class(class),
                expression.span,
                context,
                self.diagnostics,
            ) {
                return None;
            }
            let producer = lower_object_call(expression, class);
            let span = producer.span();
            return Some(crate::hir::HirObjectDestinationInitialization::Direct { producer, span });
        }

        let source = self.check_object_source(initializer, class, context)?;
        let Some(operation) = self.copy_capabilities.constructor(class).selected() else {
            self.report_unavailable_copy_operation(class, true, initializer.span());
            return None;
        };
        Some(crate::hir::HirObjectDestinationInitialization::Copy {
            source,
            operation,
            span: initializer.span(),
        })
    }

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
                .map(|element| match *element {
                    CopyPathElement::Base(base) => {
                        let base = self
                            .program
                            .class(base)
                            .expect("capability failure base must exist");
                        format!("base {}", base.name)
                    }
                    CopyPathElement::Field(field) => {
                        let declaration = self
                            .program
                            .field(field)
                            .expect("capability failure field must exist");
                        let owner = self
                            .program
                            .class(field.class())
                            .expect("capability failure owner must exist");
                        format!("{}.{}", owner.name, declaration.name)
                    }
                })
                .collect::<Vec<_>>()
                .join(" -> ");
            diagnostic = diagnostic.with_note(format!("first unavailable lifecycle path: {names}"));
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
            crate::resolve::ResolvedExpression::Construct(construction)
                if construction.class == class
        ) {
            let construction = self.check_construction_initializer(class, initializer)?;
            let elided_copy = match &construction.mode {
                crate::hir::HirConstructionMode::Initialize { .. } => {
                    // Elision does not change validity: the corresponding
                    // non-elided execution must still have a copy constructor.
                    let Some(operation) = self.copy_capabilities.constructor(class).selected()
                    else {
                        self.report_unavailable_copy_operation(class, true, initializer.span());
                        return None;
                    };
                    Some(operation)
                }
                crate::hir::HirConstructionMode::Copy { .. } => None,
            };
            return Some(HirLocalInitializer::Object(HirObjectInitialization {
                destination,
                span: construction.span,
                producer: HirObjectProducer::Construct(construction),
                elided_copy,
            }));
        }
        if matches!(
            initializer,
            crate::resolve::ResolvedExpression::Construct(construction)
                if self.program.hierarchy.is_subtype(construction.class, class) != Some(true)
        ) {
            let _ = self.check_construction_initializer(class, initializer);
            return None;
        }

        if is_object_call_source(initializer)
            && self.resolved_object_class(initializer) == Some(class)
        {
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
            let producer = lower_object_call(expression, class);
            return Some(HirLocalInitializer::Object(HirObjectInitialization {
                destination,
                span: producer.span(),
                producer,
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
        if let Some(binary) = binary_value_expression(expression) {
            let checked = self.check_binary_before_object_materialization(binary)?;
            if !require_type(
                checked.ty,
                Type::Class(class),
                checked.span,
                context,
                self.diagnostics,
            ) {
                return None;
            }
            let producer = lower_object_call(checked, class);
            return Some(HirObjectSource::Produced(producer));
        }
        if let crate::resolve::ResolvedExpression::StringLiteral(literal) = expression {
            let source = HirObjectSource::Produced(HirObjectProducer::StringLiteral(
                crate::hir::HirStringLiteral {
                    data: literal.data,
                    class: literal.class,
                    span: literal.span,
                },
            ));
            return self.convert_object_source(source, class, context);
        }
        if let crate::resolve::ResolvedExpression::StaticFieldAccess(access) = expression {
            let (place, ty) = self.check_static_place(access.field, access.span)?;
            let Type::Class(actual) = ty else {
                let _ = require_type(
                    ty,
                    Type::Class(class),
                    access.span,
                    context,
                    self.diagnostics,
                );
                return None;
            };
            return self.convert_object_source(
                HirObjectSource::Static {
                    place,
                    class: actual,
                },
                class,
                context,
            );
        }
        match expression {
            crate::resolve::ResolvedExpression::ArrayProjection(_) => {
                let checked = self.check_expression(expression)?;
                let Type::Class(_) = checked.ty else {
                    let _ = require_type(
                        checked.ty,
                        Type::Class(class),
                        checked.span,
                        context,
                        self.diagnostics,
                    );
                    return None;
                };
                let HirExpressionKind::ArrayElement(place) = checked.kind else {
                    self.diagnostics.push(
                        Diagnostic::error(
                            INVALID_OBJECT_CONTEXT,
                            "a copied array object source must be one indexed element",
                        )
                        .with_primary_label(checked.span, "slices are array values, not objects"),
                    );
                    return None;
                };
                return self.convert_object_source(
                    HirObjectSource::ArrayElement(place),
                    class,
                    context,
                );
            }
            crate::resolve::ResolvedExpression::Dereference(_) => {
                let checked = self.check_copy_construction_view(
                    expression,
                    class,
                    expression.span(),
                    expression.span(),
                )?;
                return self.finish_checked_object_source(checked, class, context);
            }
            crate::resolve::ResolvedExpression::Unwrap(_) => {
                let checked = self.check_copy_construction_view(
                    expression,
                    class,
                    expression.span(),
                    expression.span(),
                )?;
                return self.finish_checked_object_source(checked, class, context);
            }
            crate::resolve::ResolvedExpression::ObjectCast(cast) => {
                let checked = self.check_object_cast(cast)?;
                return self.finish_checked_object_source(checked, class, context);
            }
            crate::resolve::ResolvedExpression::FieldAccess(access)
                if matches!(
                    access.receiver,
                    crate::resolve::ResolvedObjectReceiver::OptionalPayload { .. }
                ) =>
            {
                let checked = self.check_copy_construction_view(
                    expression,
                    class,
                    expression.span(),
                    expression.span(),
                )?;
                return self.finish_checked_object_source(checked, class, context);
            }
            crate::resolve::ResolvedExpression::FieldAccess(access)
                if matches!(
                    access.receiver,
                    crate::resolve::ResolvedObjectReceiver::Produced { .. }
                ) =>
            {
                let field = self
                    .program
                    .field(access.field)
                    .expect("resolved produced source field must exist");
                let crate::resolve::ResolvedTypeKind::Class(field_class) = field.type_syntax.kind
                else {
                    self.diagnostics.push(
                        Diagnostic::error(
                            INVALID_OBJECT_CONTEXT,
                            "owning copy source must designate a class object",
                        )
                        .with_primary_label(
                            access.member_span,
                            "this produced field has a primitive type",
                        ),
                    );
                    return None;
                };
                let receiver =
                    access
                        .receiver
                        .clone()
                        .project_field(access.field, field_class, access.span);
                let receiver = self.check_object_receiver(&receiver, ObjectPlaceUse::CopySource)?;
                let super::super::expression::CheckedReceiverCarrier::View { view, .. } =
                    receiver.carrier
                else {
                    unreachable!("produced field copy source must retain its object view")
                };
                let checked = crate::hir::HirCheckedObjectView {
                    view: *view,
                    consumer_target: crate::hir::HirViewTarget::Class(field_class),
                    consumer_access: crate::hir::HirAccess::ReadOnly,
                    kind: crate::hir::HirCheckedObjectViewKind::Static,
                    projections: Vec::new(),
                    class: Some(field_class),
                    span: access.span,
                };
                return self.finish_checked_object_source(checked, class, context);
            }
            crate::resolve::ResolvedExpression::Grouped(grouped)
                if is_checked_object_source_expression(&grouped.expression) =>
            {
                return self.check_object_source(&grouped.expression, class, context);
            }
            crate::resolve::ResolvedExpression::FieldAccess(access)
                if access.receiver.cast().is_some() =>
            {
                let field = self
                    .program
                    .field(access.field)
                    .expect("resolved checked-source field must exist");
                let crate::resolve::ResolvedTypeKind::Class(field_class) = field.type_syntax.kind
                else {
                    self.diagnostics.push(
                        Diagnostic::error(
                            INVALID_OBJECT_CONTEXT,
                            "owning copy source must designate a class object",
                        )
                        .with_primary_label(
                            access.member_span,
                            "this cast-selected field has a primitive type",
                        ),
                    );
                    return None;
                };
                let receiver =
                    self.check_object_receiver(&access.receiver, ObjectPlaceUse::CopySource)?;
                let super::super::expression::CheckedReceiverCarrier::Checked {
                    view: mut checked,
                    ..
                } = receiver.carrier
                else {
                    unreachable!("cast-rooted field source must retain its checked view")
                };
                checked
                    .projections
                    .push(crate::object_path::ObjectProjection::Field(access.field));
                checked.class = Some(field_class);
                checked.consumer_target = crate::hir::HirViewTarget::Class(field_class);
                checked.span = access.span;
                return self.finish_checked_object_source(*checked, class, context);
            }
            _ => {}
        }
        if let Some(construction) = construction_through_groups(expression) {
            let mut construction =
                self.check_object_construction(construction.class, construction, "object source")?;
            construction.span = expression.span();
            let source = HirObjectSource::Produced(HirObjectProducer::Construct(construction));
            return self.convert_object_source(source, class, context);
        }
        if is_object_call_source(expression) {
            let checked = self.check_expression(expression)?;
            let Type::Class(actual) = checked.ty else {
                let _ = require_type(
                    checked.ty,
                    Type::Class(class),
                    checked.span,
                    context,
                    self.diagnostics,
                );
                return None;
            };
            let source = HirObjectSource::Produced(lower_object_call(checked, actual));
            return self.convert_object_source(source, class, context);
        }
        let source = self
            .check_object_source_place(expression)
            .map(HirObjectSource::Place)?;
        self.convert_object_source(source, class, context)
    }

    fn finish_checked_object_source(
        &mut self,
        checked: crate::hir::HirCheckedObjectView,
        target: ClassId,
        context: &'static str,
    ) -> Option<HirObjectSource> {
        if checked.class.is_none() {
            self.diagnostics.push(
                Diagnostic::error(
                    INVALID_OBJECT_CONTEXT,
                    "owning copy source requires a class cast",
                )
                .with_primary_label(
                    checked.span,
                    "interface and `Obj` views have no standalone inline storage",
                ),
            );
            return None;
        }
        self.convert_object_source(HirObjectSource::Checked(Box::new(checked)), target, context)
    }

    fn convert_object_source(
        &mut self,
        source: HirObjectSource,
        target: ClassId,
        context: &'static str,
    ) -> Option<HirObjectSource> {
        let actual = source.class();
        if actual == target {
            return Some(source);
        }
        let Some(true) = self.program.hierarchy.is_subtype(actual, target) else {
            let actual_name = &self
                .program
                .class(actual)
                .expect("object source class must exist")
                .name;
            let target_name = &self
                .program
                .class(target)
                .expect("object target class must exist")
                .name;
            let diagnostic = if matches!(
                source,
                HirObjectSource::Produced(HirObjectProducer::Construct(_))
            ) {
                Diagnostic::error(
                    INVALID_CONSTRUCTION,
                    format!("constructor type does not match the {context}"),
                )
                .with_primary_label(
                    source.span(),
                    format!("constructs `{actual_name}`, expected `{target_name}`"),
                )
            } else {
                Diagnostic::error(
                    INVALID_OBJECT_CONTEXT,
                    "copy source and destination must have the same class or an ancestry relation",
                )
                .with_primary_label(
                    source.span(),
                    format!("source has class `{actual_name}`, expected `{target_name}`"),
                )
            };
            self.diagnostics.push(diagnostic);
            return None;
        };
        let bases = self
            .program
            .hierarchy
            .base_chain(actual)
            .expect("valid subtype source must have valid ancestry")
            .take_while(|base| *base != target)
            .chain(std::iter::once(target))
            .collect();
        let span = source.span();
        Some(HirObjectSource::Slice(HirObjectSlice {
            source: Box::new(source),
            bases,
            target,
            span,
        }))
    }

    pub(in crate::typeck) fn resolved_object_class(
        &self,
        expression: &crate::resolve::ResolvedExpression,
    ) -> Option<ClassId> {
        match expression {
            crate::resolve::ResolvedExpression::Binding(binding) => {
                match self.binding_type(binding.binding) {
                    Type::Class(class) => Some(class),
                    _ => None,
                }
            }
            crate::resolve::ResolvedExpression::FieldAccess(access) => {
                match self
                    .program
                    .field(access.field)
                    .expect("resolved field access must select a field")
                    .type_syntax
                    .kind
                {
                    crate::resolve::ResolvedTypeKind::Class(class) => Some(class),
                    _ => None,
                }
            }
            crate::resolve::ResolvedExpression::Construct(construction) => Some(construction.class),
            crate::resolve::ResolvedExpression::StringLiteral(literal) => Some(literal.class),
            crate::resolve::ResolvedExpression::DirectCall(call) => self
                .program
                .declarations
                .get(call.function)
                .and_then(|declaration| match declaration.return_type.kind {
                    crate::resolve::ResolvedTypeKind::Class(class) => Some(class),
                    _ => None,
                }),
            crate::resolve::ResolvedExpression::IndirectCall(call) => self
                .program
                .function_types
                .get(call.function_type)
                .and_then(|signature| match signature.result.kind {
                    crate::resolve::ResolvedTypeKind::Class(class) => Some(class),
                    _ => None,
                }),
            crate::resolve::ResolvedExpression::StaticCall(call) => self
                .program
                .method(call.method)
                .and_then(|method| match method.return_type.kind {
                    crate::resolve::ResolvedTypeKind::Class(class) => Some(class),
                    _ => None,
                }),
            crate::resolve::ResolvedExpression::MethodCall(call) => self
                .program
                .method(call.method)
                .and_then(|method| match method.return_type.kind {
                    crate::resolve::ResolvedTypeKind::Class(class) => Some(class),
                    _ => None,
                }),
            crate::resolve::ResolvedExpression::InterfaceCall(call) => self
                .program
                .interface(call.interface)
                .and_then(|interface| interface.requirements.get(call.requirement.index()))
                .and_then(|requirement| match requirement.return_type.kind {
                    crate::resolve::ResolvedTypeKind::Class(class) => Some(class),
                    _ => None,
                }),
            crate::resolve::ResolvedExpression::ObjectCast(cast) => match cast.target.kind {
                crate::resolve::ResolvedTypeKind::Class(class) => Some(class),
                _ => None,
            },
            crate::resolve::ResolvedExpression::Dereference(dereference) => {
                match dereference.target {
                    crate::resolve::ResolvedSharedTarget::Class(class) => Some(class),
                    crate::resolve::ResolvedSharedTarget::Interface(_)
                    | crate::resolve::ResolvedSharedTarget::Obj
                    | crate::resolve::ResolvedSharedTarget::Array(_)
                    | crate::resolve::ResolvedSharedTarget::OptionalBox(_) => None,
                }
            }
            crate::resolve::ResolvedExpression::Grouped(grouped) => {
                self.resolved_object_class(&grouped.expression)
            }
            _ => None,
        }
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
                if parameter.binding_mode == crate::resolve::ResolvedParameterBindingMode::Value
                    || (matches!(
                        parameter.binding_mode,
                        crate::resolve::ResolvedParameterBindingMode::MutableAlias { .. }
                    ) && destination.projections().is_empty())
                {
                    true
                } else {
                    self.diagnostics.push(
                        Diagnostic::error(
                            INVALID_OBJECT_CONTEXT,
                            "an alias-rooted object cannot be replaced",
                        )
                        .with_primary_label(
                            destination.span(),
                            "assign an owning place or the complete referent of a mutable alias",
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

pub(in crate::typeck) fn is_checked_object_source_expression(
    expression: &crate::resolve::ResolvedExpression,
) -> bool {
    match expression {
        crate::resolve::ResolvedExpression::StringLiteral(_)
        | crate::resolve::ResolvedExpression::Dereference(_)
        | crate::resolve::ResolvedExpression::Unwrap(_)
        | crate::resolve::ResolvedExpression::ObjectCast(_) => true,
        crate::resolve::ResolvedExpression::Grouped(grouped) => {
            is_checked_object_source_expression(&grouped.expression)
        }
        crate::resolve::ResolvedExpression::FieldAccess(access) => {
            access.receiver.cast().is_some()
                || matches!(
                    access.receiver,
                    crate::resolve::ResolvedObjectReceiver::OptionalPayload { .. }
                        | crate::resolve::ResolvedObjectReceiver::Produced { .. }
                )
        }
        _ => false,
    }
}

fn is_object_call_source(expression: &crate::resolve::ResolvedExpression) -> bool {
    match expression {
        crate::resolve::ResolvedExpression::DirectCall(_)
        | crate::resolve::ResolvedExpression::IndirectCall(_)
        | crate::resolve::ResolvedExpression::StaticCall(_)
        | crate::resolve::ResolvedExpression::MethodCall(_)
        | crate::resolve::ResolvedExpression::InterfaceCall(_) => true,
        crate::resolve::ResolvedExpression::Unary(_)
        | crate::resolve::ResolvedExpression::Binary(_)
            if super::super::expression::is_selected_operator_expression(expression) =>
        {
            true
        }
        crate::resolve::ResolvedExpression::Grouped(grouped) => {
            is_object_call_source(&grouped.expression)
        }
        _ => false,
    }
}

fn binary_value_expression(
    expression: &crate::resolve::ResolvedExpression,
) -> Option<&crate::resolve::ResolvedBinaryExpr> {
    match expression {
        crate::resolve::ResolvedExpression::Binary(binary) => Some(binary),
        crate::resolve::ResolvedExpression::Grouped(grouped) => {
            binary_value_expression(&grouped.expression)
        }
        _ => None,
    }
}

pub(in crate::typeck) fn is_ungrouped_object_call(
    expression: &crate::resolve::ResolvedExpression,
) -> bool {
    matches!(
        expression,
        crate::resolve::ResolvedExpression::DirectCall(_)
            | crate::resolve::ResolvedExpression::IndirectCall(_)
            | crate::resolve::ResolvedExpression::StaticCall(_)
            | crate::resolve::ResolvedExpression::MethodCall(_)
            | crate::resolve::ResolvedExpression::InterfaceCall(_)
    )
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

pub(in crate::typeck) fn lower_object_call(
    expression: HirExpression,
    class: ClassId,
) -> HirObjectProducer {
    let span = expression.span;
    match expression.kind {
        HirExpressionKind::DirectCall {
            function,
            arguments,
        } => HirObjectProducer::Call(HirObjectCall {
            target: HirObjectCallTarget::Direct(function),
            arguments,
            class,
            span,
        }),
        HirExpressionKind::StaticCall { method, arguments } => {
            HirObjectProducer::Call(HirObjectCall {
                target: HirObjectCallTarget::Static(method),
                arguments,
                class,
                span,
            })
        }
        HirExpressionKind::MethodCall {
            receiver,
            target,
            arguments,
        } => HirObjectProducer::Call(HirObjectCall {
            target: HirObjectCallTarget::Method { receiver, target },
            arguments,
            class,
            span,
        }),
        HirExpressionKind::InterfaceCall {
            receiver,
            target,
            arguments,
        } => HirObjectProducer::Call(HirObjectCall {
            target: HirObjectCallTarget::Interface { receiver, target },
            arguments,
            class,
            span,
        }),
        HirExpressionKind::IndirectCall(call) => {
            debug_assert_eq!(call.result, Type::Class(class));
            HirObjectProducer::IndirectCall(call)
        }
        HirExpressionKind::Grouped(inner) => {
            let mut producer = lower_object_call(*inner, class);
            match &mut producer {
                HirObjectProducer::Call(call) => call.span = span,
                HirObjectProducer::IndirectCall(call) => call.span = span,
                HirObjectProducer::Construct(_) | HirObjectProducer::StringLiteral(_) => {
                    unreachable!("grouped call must remain a call producer")
                }
            }
            producer
        }
        _ => unreachable!("syntactic object call must type-check as a call expression"),
    }
}
