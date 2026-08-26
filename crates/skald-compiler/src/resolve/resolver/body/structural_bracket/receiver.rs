//! Static receiver classification and single-resolution carrier conversion.

use super::*;

pub(super) enum BracketReceiver {
    Intrinsic(ResolvedExpression),
    Class(ResolvedObjectReceiver),
    Interface {
        receiver: ResolvedInterfaceReceiver,
        interface: InterfaceId,
        receiver_span: Span,
    },
    Unsupported(ResolvedExpression),
    Diagnosed,
}

impl CallableResolver<'_, '_> {
    pub(super) fn classify_bracket_receiver(
        &mut self,
        receiver: ResolvedExpression,
        operator: syntax::BracketProjectionOperator,
    ) -> BracketReceiver {
        let ty = self.resolved_expression_type(&receiver);
        match (operator, ty) {
            (
                syntax::BracketProjectionOperator::Ordinary { .. },
                Some(ResolvedTypeKind::Array(_)),
            )
            | (
                syntax::BracketProjectionOperator::Shared { .. },
                Some(ResolvedTypeKind::Shared(ResolvedSharedTarget::Array(_))),
            ) => BracketReceiver::Intrinsic(receiver),
            (
                syntax::BracketProjectionOperator::Ordinary { .. },
                Some(ResolvedTypeKind::Class(class)),
            ) => match self.object_receiver_from_resolved_expression(receiver, class) {
                Some(receiver) => BracketReceiver::Class(receiver),
                None => BracketReceiver::Diagnosed,
            },
            (
                syntax::BracketProjectionOperator::Shared { arrow_span, .. },
                Some(ResolvedTypeKind::Shared(ResolvedSharedTarget::Class(class))),
            ) => {
                let span = self.cover(receiver.span(), arrow_span);
                BracketReceiver::Class(ResolvedObjectReceiver::Dereference {
                    dereference: Box::new(ResolvedDereferenceExpr {
                        source: Box::new(receiver),
                        target: ResolvedSharedTarget::Class(class),
                        operator: ResolvedDereferenceOperator::Arrow,
                        operator_span: arrow_span,
                        span,
                    }),
                    projections: Vec::new(),
                    class,
                    span,
                })
            }
            (
                syntax::BracketProjectionOperator::Ordinary { .. },
                Some(ResolvedTypeKind::Interface(interface)),
            ) => match self.interface_receiver_from_resolved_expression(receiver, interface) {
                Some((receiver, receiver_span)) => BracketReceiver::Interface {
                    receiver,
                    interface,
                    receiver_span,
                },
                None => BracketReceiver::Diagnosed,
            },
            (
                syntax::BracketProjectionOperator::Shared { arrow_span, .. },
                Some(ResolvedTypeKind::Shared(ResolvedSharedTarget::Interface(interface))),
            ) => {
                let span = self.cover(receiver.span(), arrow_span);
                BracketReceiver::Interface {
                    receiver: ResolvedInterfaceReceiver::Dereference(Box::new(
                        ResolvedDereferenceExpr {
                            source: Box::new(receiver),
                            target: ResolvedSharedTarget::Interface(interface),
                            operator: ResolvedDereferenceOperator::Arrow,
                            operator_span: arrow_span,
                            span,
                        },
                    )),
                    interface,
                    receiver_span: span,
                }
            }
            (
                syntax::BracketProjectionOperator::Ordinary { .. },
                Some(ResolvedTypeKind::Shared(
                    target @ (ResolvedSharedTarget::Class(_) | ResolvedSharedTarget::Interface(_)),
                )),
            ) => {
                self.report_implicit_shared_bracket_access(receiver.span(), target);
                BracketReceiver::Diagnosed
            }
            _ => BracketReceiver::Unsupported(receiver),
        }
    }

    fn interface_receiver_from_resolved_expression(
        &mut self,
        expression: ResolvedExpression,
        interface: InterfaceId,
    ) -> Option<(ResolvedInterfaceReceiver, Span)> {
        let span = expression.span();
        let receiver = match expression {
            ResolvedExpression::Binding(binding) => ResolvedInterfaceReceiver::Binding {
                binding: binding.binding,
                span: binding.span,
            },
            ResolvedExpression::Grouped(grouped) => {
                let (receiver, _) = self
                    .interface_receiver_from_resolved_expression(*grouped.expression, interface)?;
                return Some((receiver, grouped.span));
            }
            ResolvedExpression::ObjectCast(cast)
                if cast.target.kind == ResolvedTypeKind::Interface(interface)
                    && cast.target_mode == ResolvedObjectCastTargetMode::Plain =>
            {
                ResolvedInterfaceReceiver::Cast(Box::new(cast))
            }
            ResolvedExpression::Dereference(dereference)
                if dereference.target == ResolvedSharedTarget::Interface(interface) =>
            {
                ResolvedInterfaceReceiver::Dereference(Box::new(dereference))
            }
            ResolvedExpression::Unwrap(unwrap)
                if self.resolved_optional_box_object_leaf(&unwrap)
                    == Some(ResolvedObjectTarget::Interface(interface)) =>
            {
                ResolvedInterfaceReceiver::OptionalBoxPayload(Box::new(unwrap))
            }
            unsupported => {
                self.diagnostics.push(
                    Diagnostic::error(
                        INVALID_INDEX_PROTOCOL,
                        "this interface expression cannot be used as a structural bracket receiver",
                    )
                    .with_primary_label(unsupported.span(), "unsupported interface receiver form"),
                );
                return None;
            }
        };
        Some((receiver, span))
    }

    fn object_receiver_from_resolved_expression(
        &mut self,
        expression: ResolvedExpression,
        class: ClassId,
    ) -> Option<ResolvedObjectReceiver> {
        match ResolvedObjectReceiver::from_expression(expression, class) {
            Ok(receiver) => Some(receiver),
            Err(unsupported) => {
                self.diagnostics.push(
                    Diagnostic::error(
                        INVALID_INDEX_PROTOCOL,
                        "this class expression cannot be used as a structural index receiver",
                    )
                    .with_primary_label(unsupported.span(), "unsupported receiver form"),
                );
                None
            }
        }
    }
}
