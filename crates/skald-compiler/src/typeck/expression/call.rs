//! Direct calls, method calls, and argument binding.

use super::*;
use crate::{
    hir::{HirAccess, HirCallArgument, HirCopyArgument, HirExpressionKind},
    identity::BindingId,
    resolve::{ResolvedParameter, ResolvedParameterBindingMode},
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

    pub(super) fn check_method_call(
        &mut self,
        call: &crate::resolve::ResolvedMethodCallExpr,
    ) -> Option<HirExpression> {
        let receiver = self.check_object_place(&call.receiver, ObjectPlaceUse::Member)?;
        let method = self
            .program
            .method(call.method)
            .expect("resolved method call must reference a method");
        let mut valid = true;
        if self
            .receiver
            .is_some_and(|context| context.body_kind.initializes_receiver())
            && receiver.root() == BindingId::Receiver(self.callable)
            && receiver.path.is_root()
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
            && receiver.access == HirAccess::ReadOnly
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
        valid.then_some(HirExpression {
            kind: HirExpressionKind::MethodCall {
                receiver,
                method: call.method,
                arguments,
            },
            ty: lower_type(&method.return_type),
            span: call.span,
        })
    }

    pub(in crate::typeck) fn check_arguments(
        &mut self,
        source: &[ResolvedExpression],
        parameters: &[ResolvedParameter],
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

    fn check_argument(
        &mut self,
        source: &ResolvedExpression,
        parameter: &ResolvedParameter,
    ) -> Option<HirCallArgument> {
        match parameter.binding_mode {
            ResolvedParameterBindingMode::Value => {
                if let Type::Class(class) = lower_type(&parameter.type_syntax) {
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
                let argument = self.check_expression(source)?;
                require_type(
                    argument.ty,
                    lower_type(&parameter.type_syntax),
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
