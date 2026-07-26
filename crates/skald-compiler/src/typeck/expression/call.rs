//! Direct calls, method calls, and argument binding.

use super::*;
use crate::{
    hir::{
        HirAccess, HirCallArgument, HirCopyArgument, HirExpressionKind, HirInterfaceCallTarget,
        HirInterfaceReceiver, HirMethodCallTarget, HirMethodReceiver, HirObjectOrigin,
        HirObjectView, HirViewSource, HirViewTarget,
    },
    identity::BindingId,
    resolve::{ResolvedMethodDispatch, ResolvedParameterBindingMode},
};

use crate::typeck::program::{
    lower_type, INVALID_INITIALIZER_BODY, READ_ONLY_RECEIVER, WRONG_ARGUMENT_COUNT,
};

impl CallableChecker<'_, '_> {
    pub(super) fn check_direct_call(
        &mut self,
        call: &crate::resolve::ResolvedDirectCallExpr,
    ) -> Option<HirExpression> {
        let target = self
            .program
            .declarations
            .get(call.function)
            .expect("resolved direct-call target must exist");
        let arguments = self.check_arguments(
            &call.arguments,
            &target.parameters,
            call.callee_span,
            "function",
            Some(&target.name),
            Some(target.name_span),
        )?;
        Some(HirExpression {
            kind: HirExpressionKind::DirectCall {
                function: call.function,
                arguments,
            },
            ty: lower_type(&target.return_type),
            span: call.span,
        })
    }

    pub(super) fn check_interface_call(
        &mut self,
        call: &crate::resolve::ResolvedInterfaceCallExpr,
    ) -> Option<HirExpression> {
        let interface = self
            .program
            .interface(call.interface)
            .expect("resolved interface call must reference an interface");
        let requirement = interface
            .requirements
            .get(call.requirement.index())
            .filter(|requirement| requirement.id == call.requirement)
            .expect("resolved interface call must reference a requirement");
        let required_access = if requirement.mutable {
            HirAccess::Mutable
        } else {
            HirAccess::ReadOnly
        };
        let (access, receiver) = match &call.receiver {
            crate::resolve::ResolvedInterfaceReceiver::Binding { binding, span } => {
                let target = HirViewTarget::Interface(call.interface);
                let access = self.binding_access(*binding, false, *span)?;
                let view = HirObjectView {
                    source: HirViewSource::Forwarded {
                        binding: *binding,
                        target,
                        access,
                        span: *span,
                    },
                    origin: Box::new(HirObjectOrigin::Forwarded {
                        binding: *binding,
                        static_target: target,
                        access,
                        dispatch_limit: None,
                        span: *span,
                    }),
                    target,
                    access,
                    span: *span,
                };
                (access, HirInterfaceReceiver::View(view))
            }
            crate::resolve::ResolvedInterfaceReceiver::Cast(cast) => {
                let mut checked = self.check_object_cast(cast)?;
                debug_assert_eq!(
                    checked.view.target,
                    HirViewTarget::Interface(call.interface)
                );
                let access = checked.view.access;
                checked.consumer_access = required_access;
                (access, HirInterfaceReceiver::Checked(Box::new(checked)))
            }
            crate::resolve::ResolvedInterfaceReceiver::Dereference(dereference) => {
                let target = HirViewTarget::Interface(call.interface);
                let pointee = self.check_explicit_shared_pointee(
                    dereference,
                    Vec::new(),
                    call.receiver_span,
                )?;
                let access = pointee.access();
                let view = pointee.into_view(target, access);
                (access, HirInterfaceReceiver::View(view))
            }
        };
        if !access.permits(required_access) {
            self.diagnostics.push(
                Diagnostic::error(
                    READ_ONLY_RECEIVER,
                    format!(
                        "mutable interface requirement `{}` requires mutable receiver access",
                        requirement.name
                    ),
                )
                .with_primary_label(
                    call.member_span,
                    "called through a read-only interface view",
                )
                .with_secondary_label(requirement.name_span, "mutable requirement declared here"),
            );
            return None;
        }
        let arguments = self.check_arguments(
            &call.arguments,
            &requirement.parameters,
            call.member_span,
            "interface requirement",
            Some(&requirement.name),
            Some(requirement.name_span),
        )?;
        Some(HirExpression {
            kind: HirExpressionKind::InterfaceCall {
                receiver,
                target: HirInterfaceCallTarget {
                    interface: call.interface,
                    requirement: call.requirement,
                },
                arguments,
            },
            ty: lower_type(&requirement.return_type),
            span: call.span,
        })
    }

    pub(super) fn check_method_call(
        &mut self,
        call: &crate::resolve::ResolvedMethodCallExpr,
    ) -> Option<HirExpression> {
        let receiver = self.check_object_receiver(&call.receiver, ObjectPlaceUse::Member)?;
        let method = self
            .program
            .method(call.method)
            .expect("resolved method call must reference a method");
        let mut valid = true;
        if self
            .receiver
            .is_some_and(|context| context.body_kind.initializes_receiver())
            && receiver.place.root() == BindingId::Receiver(self.callable)
            && receiver.place.path.is_root()
        {
            self.diagnostics.push(
                Diagnostic::error(
                    INVALID_INITIALIZER_BODY,
                    "an initializer cannot call instance methods",
                )
                .with_primary_label(call.member_span, "the complete receiver is not live yet"),
            );
            valid = false;
        }
        if method.receiver_access == crate::resolve::ResolvedReceiverAccess::Mutable
            && receiver.place.access == HirAccess::ReadOnly
        {
            self.diagnostics.push(
                Diagnostic::error(
                    READ_ONLY_RECEIVER,
                    format!(
                        "mutable method `{}` requires mutable receiver access",
                        method.name
                    ),
                )
                .with_primary_label(call.member_span, "called through a read-only receiver")
                .with_secondary_label(method.name_span, "mutable method declared here"),
            );
            valid = false;
        }
        let arguments = self.check_arguments(
            &call.arguments,
            &method.parameters,
            call.member_span,
            "method",
            Some(&method.name),
            Some(method.name_span),
        )?;
        let target = match method.dispatch {
            ResolvedMethodDispatch::Direct => HirMethodCallTarget::Direct(call.method),
            ResolvedMethodDispatch::VirtualRoot { family, slot }
            | ResolvedMethodDispatch::Override { family, slot, .. } => {
                if matches!(
                    receiver.origin,
                    HirObjectOrigin::Exact { .. }
                        | HirObjectOrigin::Produced { .. }
                        | HirObjectOrigin::Forwarded {
                            dispatch_limit: Some(_),
                            ..
                        }
                ) {
                    HirMethodCallTarget::Direct(call.method)
                } else {
                    HirMethodCallTarget::Virtual {
                        family,
                        slot,
                        selected: call.method,
                    }
                }
            }
        };
        valid.then_some(HirExpression {
            kind: HirExpressionKind::MethodCall {
                receiver: HirMethodReceiver {
                    place: receiver.place,
                    origin: Box::new(receiver.origin),
                    checked_cast: receiver.checked_cast,
                    shared_view: receiver.shared_view,
                    optional_view: receiver.optional_view,
                    array_element: receiver.array_element,
                },
                target,
                arguments,
            },
            ty: lower_type(&method.return_type),
            span: call.span,
        })
    }

    pub(in crate::typeck) fn check_arguments<P: CallParameter>(
        &mut self,
        source: &[ResolvedExpression],
        parameters: &[P],
        target_span: Span,
        target_kind: &'static str,
        target_name: Option<&str>,
        declaration_span: Option<Span>,
    ) -> Option<Vec<HirCallArgument>> {
        let mut arguments = Vec::with_capacity(source.len());
        let mut valid = true;
        for (index, argument) in source.iter().enumerate() {
            match parameters.get(index) {
                Some(parameter) => match self.check_argument(argument, parameter) {
                    Some(argument) => arguments.push(argument),
                    None => valid = false,
                },
                None => {
                    let _ = self.check_expression(argument);
                    valid = false;
                }
            }
        }
        if source.len() != parameters.len() {
            let target = target_name
                .map(|name| format!("{target_kind} `{name}`"))
                .unwrap_or_else(|| target_kind.to_owned());
            let mut diagnostic = Diagnostic::error(
                WRONG_ARGUMENT_COUNT,
                format!(
                    "{target} expects {} argument{} but received {}",
                    parameters.len(),
                    if parameters.len() == 1 { "" } else { "s" },
                    source.len()
                ),
            )
            .with_primary_label(target_span, "called with the wrong number of arguments");
            if let Some(declaration_span) = declaration_span {
                diagnostic = diagnostic
                    .with_secondary_label(declaration_span, format!("{target_kind} declared here"));
            }
            self.diagnostics.push(diagnostic);
            valid = false;
        }
        valid.then_some(arguments)
    }

    fn check_argument<P: CallParameter>(
        &mut self,
        source: &ResolvedExpression,
        parameter: &P,
    ) -> Option<HirCallArgument> {
        match parameter.binding_mode() {
            ResolvedParameterBindingMode::Value => {
                let parameter_type = lower_type(parameter.type_syntax());
                if let Type::Shared(target) = parameter_type {
                    return self
                        .check_shared_transfer(source, target, "shared value argument")
                        .map(HirCallArgument::Shared);
                }
                if let Type::Class(class) = parameter_type {
                    let source =
                        self.check_object_source(source, class, "object value argument")?;
                    let Some(operation) = self.copy_capabilities.constructor(class).selected()
                    else {
                        self.report_unavailable_copy_operation(class, true, source.span());
                        return None;
                    };
                    return Some(HirCallArgument::Copy(HirCopyArgument {
                        span: source.span(),
                        source,
                        operation,
                    }));
                }
                if let Type::Array(array) = parameter_type {
                    return self
                        .check_array_initialize(array, source, "array value argument")
                        .map(HirCallArgument::Array);
                }
                if let Type::OptionalPrimitive(payload) = parameter_type {
                    return self
                        .check_optional_source(source, payload, "primitive optional argument")
                        .map(|source| HirCallArgument::Optional { source, payload });
                }
                if let Type::OptionalClass(class) = parameter_type {
                    return self
                        .check_class_optional_initialize(class, source, "class optional argument")
                        .map(HirCallArgument::ClassOptional);
                }
                if let Type::OptionalShared(target) = parameter_type {
                    return self
                        .check_optional_shared_initialize(
                            target,
                            source,
                            "optional shared argument",
                        )
                        .map(HirCallArgument::OptionalShared);
                }
                let argument = self.check_expression(source)?;
                require_type(
                    argument.ty,
                    parameter_type,
                    argument.span,
                    "call argument",
                    self.diagnostics,
                )
                .then_some(HirCallArgument::Value(argument))
            }
            ResolvedParameterBindingMode::ReadOnlyAlias { .. }
            | ResolvedParameterBindingMode::MutableAlias { .. } => {
                self.check_alias_argument(source, parameter)
            }
        }
    }
}
