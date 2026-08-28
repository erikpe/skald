//! Realization of canonical bound calls for primitive specializations.

use super::*;

impl CallableResolver<'_, '_> {
    pub(super) fn resolve_primitive_bound_call(
        &mut self,
        receiver: ResolvedExpression,
        operation: ResolvedPrimitiveBoundOperation,
        member_span: Span,
        arguments: Vec<ResolvedExpression>,
        span: Span,
    ) -> Option<ResolvedExpression> {
        match operation {
            ResolvedPrimitiveBoundOperation::Operator(operation) => self
                .resolve_primitive_operator_call(receiver, operation, member_span, arguments, span),
            ResolvedPrimitiveBoundOperation::Successor(primitive) => self
                .resolve_primitive_successor_call(
                    receiver,
                    primitive,
                    member_span,
                    arguments,
                    span,
                ),
        }
    }

    fn resolve_primitive_operator_call(
        &mut self,
        receiver: ResolvedExpression,
        operation: ResolvedPrimitiveOperatorOperation,
        member_span: Span,
        mut arguments: Vec<ResolvedExpression>,
        span: Span,
    ) -> Option<ResolvedExpression> {
        let protocol = operation.protocol();
        match protocol.shape() {
            CanonicalOperatorProtocolShape::Unary if arguments.is_empty() => {
                Some(ResolvedExpression::Unary(ResolvedUnaryExpr {
                    operator: unary_operator(protocol),
                    operator_span: member_span,
                    operand: Box::new(receiver),
                    selection: None,
                    span,
                }))
            }
            CanonicalOperatorProtocolShape::Binary | CanonicalOperatorProtocolShape::Predicate
                if arguments.len() == 1 =>
            {
                let right = arguments.pop().expect("one primitive operator argument");
                Some(ResolvedExpression::Binary(ResolvedBinaryExpr {
                    left: Box::new(receiver),
                    operator: binary_operator(protocol),
                    operator_span: member_span,
                    right: Box::new(right),
                    selection: None,
                    span,
                }))
            }
            _ => {
                let expected = usize::from(!matches!(
                    protocol.shape(),
                    CanonicalOperatorProtocolShape::Unary
                ));
                self.diagnostics.push(
                    Diagnostic::error(
                        INVALID_CALL_TARGET,
                        format!(
                            "operator requirement `{}` expects {expected} argument{}",
                            protocol.requirement_name(),
                            if expected == 1 { "" } else { "s" }
                        ),
                    )
                    .with_primary_label(member_span, "wrong number of arguments"),
                );
                None
            }
        }
    }

    fn resolve_primitive_successor_call(
        &mut self,
        receiver: ResolvedExpression,
        primitive: ResolvedPrimitiveType,
        member_span: Span,
        arguments: Vec<ResolvedExpression>,
        span: Span,
    ) -> Option<ResolvedExpression> {
        if !arguments.is_empty() {
            self.diagnostics.push(
                Diagnostic::error(
                    INVALID_CALL_TARGET,
                    "successor requirement expects no arguments",
                )
                .with_primary_label(member_span, "wrong number of arguments"),
            );
            return None;
        }
        let (kind, spelling) = match primitive {
            ResolvedPrimitiveType::I64 => (
                crate::literal::NumericLiteralKind::I64(crate::literal::IntegerRadix::Decimal),
                "1",
            ),
            ResolvedPrimitiveType::U64 => (
                crate::literal::NumericLiteralKind::U64(crate::literal::IntegerRadix::Decimal),
                "1u",
            ),
            ResolvedPrimitiveType::U8 => (
                crate::literal::NumericLiteralKind::U8(crate::literal::IntegerRadix::Decimal),
                "1u8",
            ),
            ResolvedPrimitiveType::F64 | ResolvedPrimitiveType::Bool => {
                unreachable!("successor evidence is integer-only")
            }
        };
        let one = ResolvedExpression::NumericLiteral(ResolvedNumericLiteralExpr {
            kind,
            spelling: spelling.to_owned(),
            span: member_span,
        });
        Some(ResolvedExpression::Binary(ResolvedBinaryExpr {
            left: Box::new(receiver),
            operator: ResolvedBinaryOperator::Add,
            operator_span: member_span,
            right: Box::new(one),
            selection: None,
            span,
        }))
    }
}

fn unary_operator(protocol: CanonicalOperatorProtocol) -> ResolvedUnaryOperator {
    match protocol {
        CanonicalOperatorProtocol::Neg => ResolvedUnaryOperator::Negate,
        CanonicalOperatorProtocol::BitNot => ResolvedUnaryOperator::BitwiseComplement,
        _ => unreachable!("primitive unary realization uses a unary protocol"),
    }
}

fn binary_operator(protocol: CanonicalOperatorProtocol) -> ResolvedBinaryOperator {
    match protocol {
        CanonicalOperatorProtocol::Eq => ResolvedBinaryOperator::Equal,
        CanonicalOperatorProtocol::Less => ResolvedBinaryOperator::LessThan,
        CanonicalOperatorProtocol::LessEq => ResolvedBinaryOperator::LessEqual,
        CanonicalOperatorProtocol::Greater => ResolvedBinaryOperator::GreaterThan,
        CanonicalOperatorProtocol::GreaterEq => ResolvedBinaryOperator::GreaterEqual,
        CanonicalOperatorProtocol::Add => ResolvedBinaryOperator::Add,
        CanonicalOperatorProtocol::Sub => ResolvedBinaryOperator::Subtract,
        CanonicalOperatorProtocol::Mul => ResolvedBinaryOperator::Multiply,
        CanonicalOperatorProtocol::Div => ResolvedBinaryOperator::Divide,
        CanonicalOperatorProtocol::Rem => ResolvedBinaryOperator::Remainder,
        CanonicalOperatorProtocol::BitAnd => ResolvedBinaryOperator::BitwiseAnd,
        CanonicalOperatorProtocol::BitOr => ResolvedBinaryOperator::BitwiseOr,
        CanonicalOperatorProtocol::BitXor => ResolvedBinaryOperator::BitwiseXor,
        CanonicalOperatorProtocol::ShiftLeft => ResolvedBinaryOperator::ShiftLeft,
        CanonicalOperatorProtocol::ShiftRight => ResolvedBinaryOperator::ShiftRight,
        CanonicalOperatorProtocol::Neg | CanonicalOperatorProtocol::BitNot => {
            unreachable!("unary protocols do not produce binary operators")
        }
    }
}
