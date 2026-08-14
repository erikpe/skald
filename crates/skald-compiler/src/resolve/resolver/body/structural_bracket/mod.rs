//! Array-fast-path and structural protocol selection for bracket syntax.

use super::*;

mod protocol;
mod receiver;

use protocol::StructuralBracketProtocol;
use receiver::BracketReceiver;

impl CallableResolver<'_, '_> {
    pub(super) fn resolve_bracket_projection(
        &mut self,
        projection: &syntax::BracketProjectionExpr,
    ) -> Option<ResolvedExpression> {
        let receiver = self.resolve_expression(&projection.receiver)?;

        // Structural slicing is not enabled yet. Keeping it on the existing
        // node also preserves all current array diagnostics and lowering.
        if matches!(
            projection.bounds,
            syntax::BracketProjectionBounds::Slice { .. }
        ) {
            return self.resolve_intrinsic_bracket_projection(receiver, projection);
        }

        match self.classify_bracket_receiver(receiver, projection.operator) {
            BracketReceiver::Intrinsic(receiver)
            | BracketReceiver::Interface(receiver)
            | BracketReceiver::Unsupported(receiver) => {
                self.resolve_intrinsic_bracket_projection(receiver, projection)
            }
            BracketReceiver::Structural(receiver) => {
                let method = self.select_index_protocol_method(
                    receiver.class(),
                    StructuralBracketProtocol::IndexGet,
                    projection.left_bracket_span(),
                )?;
                let receiver = self.project_receiver_to_declaring_class(receiver, method.class());
                let syntax::BracketProjectionBounds::Index(index) = &projection.bounds else {
                    unreachable!("slice projections return through the intrinsic path")
                };
                let index = self.resolve_expression(index)?;
                Some(ResolvedExpression::MethodCall(ResolvedMethodCallExpr {
                    receiver,
                    method,
                    member_span: projection.left_bracket_span(),
                    arguments: vec![index],
                    span: projection.span,
                }))
            }
            BracketReceiver::Diagnosed => None,
        }
    }

    pub(super) fn resolve_bracket_assignment(
        &mut self,
        assignment: &syntax::ObjectAssignmentStatement,
    ) -> Option<ResolvedStatement> {
        let projection = bracket_projection_through_groups(&assignment.place)
            .expect("bracket assignment dispatch requires a bracket projection");

        if matches!(
            projection.bounds,
            syntax::BracketProjectionBounds::Slice { .. }
        ) {
            return self.resolve_intrinsic_bracket_assignment(assignment);
        }

        let receiver = self.resolve_expression(&projection.receiver)?;
        match self.classify_bracket_receiver(receiver, projection.operator) {
            BracketReceiver::Intrinsic(receiver)
            | BracketReceiver::Interface(receiver)
            | BracketReceiver::Unsupported(receiver) => {
                let destination =
                    self.resolve_intrinsic_bracket_projection(receiver, projection)?;
                let destination = restore_projection_groups(&assignment.place, destination);
                let source = self.resolve_expression(&assignment.value)?;
                Some(ResolvedStatement::ArrayAssignment(
                    ResolvedArrayAssignment {
                        destination,
                        equal_span: assignment.equal_span,
                        source,
                        span: assignment.span,
                    },
                ))
            }
            BracketReceiver::Structural(receiver) => {
                let method = self.select_index_protocol_method(
                    receiver.class(),
                    StructuralBracketProtocol::IndexSet,
                    projection.left_bracket_span(),
                )?;
                let receiver = self.project_receiver_to_declaring_class(receiver, method.class());
                let syntax::BracketProjectionBounds::Index(index) = &projection.bounds else {
                    unreachable!("slice assignments return through the intrinsic path")
                };
                let index = self.resolve_expression(index)?;
                let replacement = self.resolve_expression(&assignment.value)?;
                let expression = ResolvedExpression::MethodCall(ResolvedMethodCallExpr {
                    receiver,
                    method,
                    member_span: projection.left_bracket_span(),
                    arguments: vec![index, replacement],
                    span: assignment.span,
                });
                Some(ResolvedStatement::Expression(ResolvedExpressionStatement {
                    expression,
                    span: assignment.span,
                }))
            }
            BracketReceiver::Diagnosed => None,
        }
    }

    fn resolve_intrinsic_bracket_assignment(
        &mut self,
        assignment: &syntax::ObjectAssignmentStatement,
    ) -> Option<ResolvedStatement> {
        let destination = self.resolve_expression(&assignment.place)?;
        let source = self.resolve_expression(&assignment.value)?;
        Some(ResolvedStatement::ArrayAssignment(
            ResolvedArrayAssignment {
                destination,
                equal_span: assignment.equal_span,
                source,
                span: assignment.span,
            },
        ))
    }

    fn resolve_intrinsic_bracket_projection(
        &mut self,
        receiver: ResolvedExpression,
        projection: &syntax::BracketProjectionExpr,
    ) -> Option<ResolvedExpression> {
        let operator = match projection.operator {
            syntax::BracketProjectionOperator::Ordinary { left_bracket_span } => {
                ResolvedArrayProjectionOperator::Ordinary { left_bracket_span }
            }
            syntax::BracketProjectionOperator::Shared {
                arrow_span,
                left_bracket_span,
            } => ResolvedArrayProjectionOperator::Shared {
                arrow_span,
                left_bracket_span,
            },
        };
        let bounds = match &projection.bounds {
            syntax::BracketProjectionBounds::Index(index) => {
                ResolvedArrayProjectionBounds::Index(Box::new(self.resolve_expression(index)?))
            }
            syntax::BracketProjectionBounds::Slice {
                start,
                colon_span,
                end,
            } => ResolvedArrayProjectionBounds::Slice {
                start: match start {
                    Some(bound) => Some(Box::new(self.resolve_expression(bound)?)),
                    None => None,
                },
                colon_span: *colon_span,
                end: match end {
                    Some(bound) => Some(Box::new(self.resolve_expression(bound)?)),
                    None => None,
                },
            },
        };
        Some(ResolvedExpression::ArrayProjection(Box::new(
            ResolvedArrayProjectionExpr {
                receiver: Box::new(receiver),
                operator,
                bounds,
                right_bracket_span: projection.right_bracket_span,
                span: projection.span,
            },
        )))
    }
}

impl syntax::BracketProjectionExpr {
    fn left_bracket_span(&self) -> Span {
        match self.operator {
            syntax::BracketProjectionOperator::Ordinary { left_bracket_span }
            | syntax::BracketProjectionOperator::Shared {
                left_bracket_span, ..
            } => left_bracket_span,
        }
    }
}

fn bracket_projection_through_groups(
    expression: &syntax::Expression,
) -> Option<&syntax::BracketProjectionExpr> {
    match expression {
        syntax::Expression::BracketProjection(projection) => Some(projection),
        syntax::Expression::Grouped(grouped) => {
            bracket_projection_through_groups(&grouped.expression)
        }
        _ => None,
    }
}

fn restore_projection_groups(
    syntax: &syntax::Expression,
    resolved: ResolvedExpression,
) -> ResolvedExpression {
    match syntax {
        syntax::Expression::Grouped(grouped) => {
            let expression = restore_projection_groups(&grouped.expression, resolved);
            ResolvedExpression::Grouped(ResolvedGroupedExpr {
                expression: Box::new(expression),
                span: grouped.span,
            })
        }
        syntax::Expression::BracketProjection(_) => resolved,
        _ => unreachable!("only grouping may wrap a bracket-assignment destination"),
    }
}
