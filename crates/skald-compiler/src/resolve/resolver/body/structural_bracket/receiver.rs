//! Static receiver classification and single-resolution carrier conversion.

use super::*;

pub(super) enum BracketReceiver {
    Intrinsic(ResolvedExpression),
    Structural(ResolvedObjectReceiver),
    Interface(ResolvedExpression),
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
                Some(receiver) => BracketReceiver::Structural(receiver),
                None => BracketReceiver::Diagnosed,
            },
            (
                syntax::BracketProjectionOperator::Shared { arrow_span, .. },
                Some(ResolvedTypeKind::Shared(ResolvedSharedTarget::Class(class))),
            ) => {
                let span = self.cover(receiver.span(), arrow_span);
                BracketReceiver::Structural(ResolvedObjectReceiver::Dereference {
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
                Some(ResolvedTypeKind::Interface(_)),
            )
            | (
                syntax::BracketProjectionOperator::Shared { .. },
                Some(ResolvedTypeKind::Shared(ResolvedSharedTarget::Interface(_))),
            ) => BracketReceiver::Interface(receiver),
            (
                syntax::BracketProjectionOperator::Ordinary { .. },
                Some(ResolvedTypeKind::Shared(target @ ResolvedSharedTarget::Class(_))),
            ) => {
                self.report_implicit_shared_member_access(receiver.span(), target);
                BracketReceiver::Diagnosed
            }
            _ => BracketReceiver::Unsupported(receiver),
        }
    }

    fn object_receiver_from_resolved_expression(
        &mut self,
        expression: ResolvedExpression,
        class: ClassId,
    ) -> Option<ResolvedObjectReceiver> {
        Some(match expression {
            ResolvedExpression::Binding(binding) => ResolvedObjectReceiver::from_place(
                ResolvedObjectPlace::root(binding.binding, class, binding.span),
            ),
            ResolvedExpression::Grouped(grouped) => {
                return self
                    .object_receiver_from_resolved_expression(*grouped.expression, class)
                    .map(|receiver| receiver.with_span(grouped.span));
            }
            ResolvedExpression::Dereference(dereference) => {
                let span = dereference.span;
                ResolvedObjectReceiver::Dereference {
                    dereference: Box::new(dereference),
                    projections: Vec::new(),
                    class,
                    span,
                }
            }
            ResolvedExpression::Unwrap(unwrap) => {
                ResolvedObjectReceiver::from_optional_payload(unwrap, class)
            }
            ResolvedExpression::ObjectCast(cast) => ResolvedObjectReceiver::from_cast(cast, class),
            ResolvedExpression::ArrayProjection(projection) => {
                let span = projection.span;
                ResolvedObjectReceiver::ArrayElement {
                    projection,
                    projections: Vec::new(),
                    class,
                    span,
                }
            }
            ResolvedExpression::FieldAccess(access) => {
                access
                    .receiver
                    .project_field(access.field, class, access.span)
            }
            ResolvedExpression::StaticFieldAccess(access) => {
                self.diagnostics.push(
                    Diagnostic::error(
                        INVALID_INDEX_PROTOCOL,
                        "a static field cannot be used directly as a structural index receiver",
                    )
                    .with_primary_label(
                        access.span,
                        "store this class value in a local before indexing it",
                    ),
                );
                return None;
            }
            producer @ (ResolvedExpression::StringLiteral(_)
            | ResolvedExpression::DirectCall(_)
            | ResolvedExpression::StaticCall(_)
            | ResolvedExpression::MethodCall(_)
            | ResolvedExpression::InterfaceCall(_)
            | ResolvedExpression::Construct(_)) => {
                ResolvedObjectReceiver::from_produced(producer, class)
            }
            unsupported => {
                self.diagnostics.push(
                    Diagnostic::error(
                        INVALID_INDEX_PROTOCOL,
                        "this class expression cannot be used as a structural index receiver",
                    )
                    .with_primary_label(unsupported.span(), "unsupported receiver form"),
                );
                return None;
            }
        })
    }
}
