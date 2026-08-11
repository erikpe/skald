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

use crate::typeck::function::MemberBodyKind;
use crate::typeck::program::{
    lower_type, INVALID_INITIALIZER_BODY, PANIC_REQUIRES_CALL_STATEMENT, READ_ONLY_RECEIVER,
    WRONG_ARGUMENT_COUNT,
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
        if let crate::resolve::ResolvedFunctionLinkage::Intrinsic { intrinsic } = target.linkage {
            match intrinsic {
                crate::intrinsic::Intrinsic::Panic => {
                    self.diagnostics.push(
                        crate::diagnostics::Diagnostic::error(
                            PANIC_REQUIRES_CALL_STATEMENT,
                            "`std::error::panic` can only be used as a call statement",
                        )
                        .with_primary_label(
                            call.span,
                            "panic cannot be used as a value-producing expression",
                        )
                        .with_note("write `panic(message);` as a standalone statement"),
                    );
                    return None;
                }
                crate::intrinsic::Intrinsic::F64ToBits
                | crate::intrinsic::Intrinsic::F64FromBits => {
                    return self.check_bit_reinterpretation_intrinsic_call(call, target, intrinsic);
                }
                _ => return self.check_io_intrinsic_call(call, target, intrinsic),
            }
        }
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
            ty: lower_type(self.program, &target.return_type),
            span: call.span,
        })
    }

    pub(super) fn check_static_call(
        &mut self,
        call: &crate::resolve::ResolvedStaticCallExpr,
    ) -> Option<HirExpression> {
        let target = self
            .program
            .method(call.method)
            .expect("resolved static-call target must exist");
        debug_assert_eq!(target.kind, crate::resolve::ResolvedMethodKind::Static);
        let arguments = self.check_arguments(
            &call.arguments,
            &target.parameters,
            call.member_span,
            "static method",
            Some(&target.name),
            Some(target.name_span),
        )?;
        Some(HirExpression {
            kind: HirExpressionKind::StaticCall {
                method: call.method,
                arguments,
            },
            ty: lower_type(self.program, &target.return_type),
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
            crate::resolve::ResolvedInterfaceReceiver::OptionalBoxPayload(unwrap) => {
                let view = self.check_optional_box_object_view(unwrap)?;
                let access = view.access;
                let target = HirViewTarget::Interface(call.interface);
                let view =
                    super::optional_box_view::into_object_view(view, target, access, Vec::new());
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
            ty: lower_type(self.program, &requirement.return_type),
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
            .member_body_kind
            .is_some_and(MemberBodyKind::initializes_receiver)
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
        let receiver_access = method
            .kind
            .receiver_access()
            .expect("resolved object-selected methods must be instance methods");
        if receiver_access == crate::resolve::ResolvedReceiverAccess::Mutable
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
        let target = match method
            .kind
            .dispatch()
            .expect("instance method must carry dispatch")
        {
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
            ty: lower_type(self.program, &method.return_type),
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
                let parameter_type = lower_type(self.program, parameter.type_syntax());
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
                if let Some(optional) = self.optional_kind(parameter_type) {
                    return match optional {
                        super::super::optional_types::OptionalPayloadKind::Primitive(payload) => {
                            self.check_optional_source(
                                source,
                                payload,
                                "primitive optional argument",
                            )
                            .map(|source| HirCallArgument::Optional { source, payload })
                        }
                        super::super::optional_types::OptionalPayloadKind::Class(class) => self
                            .check_class_optional_initialize(
                                class,
                                source,
                                "class optional argument",
                            )
                            .map(HirCallArgument::ClassOptional),
                        super::super::optional_types::OptionalPayloadKind::Shared(target) => self
                            .check_optional_shared_initialize(
                                target,
                                source,
                                "optional shared argument",
                            )
                            .map(HirCallArgument::OptionalShared),
                        super::super::optional_types::OptionalPayloadKind::Nested(_)
                        | super::super::optional_types::OptionalPayloadKind::Array(_) => {
                            let Type::Optional(optional) = parameter_type else {
                                unreachable!()
                            };
                            self.check_optional_value(
                                optional,
                                source,
                                "aggregate optional argument",
                            )
                            .map(HirCallArgument::AggregateOptional)
                        }
                    };
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
