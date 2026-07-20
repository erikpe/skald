//! Expression, call, binding, and primitive-operation checking.

use crate::{
    diagnostics::{Diagnostic, Diagnostics},
    hir::{HirBinaryOperation, HirExpression, HirExpressionKind, HirUnaryOperation, Type},
    identity::BindingId,
    resolve::{ResolvedBinaryOperator, ResolvedExpression, ResolvedUnaryOperator},
    source::Span,
};

use super::{
    function::FunctionChecker,
    literal::{classify_i64_magnitude, i64_literal_through_groups, Magnitude},
    program::{lower_type, TYPE_MISMATCH, WRONG_ARGUMENT_COUNT},
};

impl FunctionChecker<'_, '_> {
    pub(super) fn check_expression(
        &mut self,
        expression: &ResolvedExpression,
    ) -> Option<HirExpression> {
        match expression {
            ResolvedExpression::Binding(binding) => {
                let ty = self.binding_type(binding.binding);
                Some(HirExpression {
                    kind: HirExpressionKind::Binding(binding.binding),
                    ty,
                    span: binding.span,
                })
            }
            ResolvedExpression::NumericLiteral(literal) => self.check_numeric_literal(literal),
            ResolvedExpression::Boolean(boolean) => Some(HirExpression {
                kind: HirExpressionKind::Boolean(boolean.value),
                ty: Type::Bool,
                span: boolean.span,
            }),
            ResolvedExpression::Unary(unary) => {
                if unary.operator == ResolvedUnaryOperator::Negate {
                    if let Some(literal) = i64_literal_through_groups(&unary.operand) {
                        match classify_i64_magnitude(&literal.spelling) {
                            Magnitude::MinimumBoundary => {
                                return Some(HirExpression {
                                    kind: HirExpressionKind::I64(i64::MIN),
                                    ty: Type::I64,
                                    span: unary.span,
                                });
                            }
                            Magnitude::TooLarge => {
                                self.report_integer_out_of_range(
                                    unary.span,
                                    format!("-{}", literal.spelling),
                                );
                                return None;
                            }
                            Magnitude::PositiveI64 => {}
                        }
                    }
                }

                let operand = self.check_expression(&unary.operand)?;
                let operation = match operand.ty {
                    Type::I64 => HirUnaryOperation::NegateI64,
                    Type::F64 => HirUnaryOperation::NegateF64,
                    _ => {
                        require_type(
                            operand.ty,
                            Type::I64,
                            operand.span,
                            "unary negation operand",
                            self.diagnostics,
                        );
                        return None;
                    }
                };
                let ty = operand.ty;
                Some(HirExpression {
                    kind: HirExpressionKind::Unary {
                        operation,
                        operand: Box::new(operand),
                    },
                    ty,
                    span: unary.span,
                })
            }
            ResolvedExpression::Binary(binary) => {
                let left = self.check_expression(&binary.left);
                let right = self.check_expression(&binary.right);
                let (left, right) = match (left, right) {
                    (Some(left), Some(right)) => (left, right),
                    _ => return None,
                };
                let operation = if left.ty == right.ty {
                    select_binary_operation(binary.operator, left.ty)
                } else {
                    None
                };
                let Some(operation) = operation else {
                    let expected =
                        if matches!(left.ty, Type::I64 | Type::U64 | Type::U8 | Type::F64) {
                            left.ty
                        } else {
                            Type::I64
                        };
                    let left_valid = require_type(
                        left.ty,
                        expected,
                        left.span,
                        "left arithmetic operand",
                        self.diagnostics,
                    );
                    let right_valid = require_type(
                        right.ty,
                        expected,
                        right.span,
                        "right arithmetic operand",
                        self.diagnostics,
                    );
                    debug_assert!(!left_valid || !right_valid);
                    return None;
                };
                let ty = left.ty;

                Some(HirExpression {
                    kind: HirExpressionKind::Binary {
                        operation,
                        left: Box::new(left),
                        right: Box::new(right),
                    },
                    ty,
                    span: binary.span,
                })
            }
            ResolvedExpression::DirectCall(call) => {
                let mut arguments = Vec::with_capacity(call.arguments.len());
                let mut valid = true;
                for argument in &call.arguments {
                    match self.check_expression(argument) {
                        Some(argument) => arguments.push(argument),
                        None => valid = false,
                    }
                }

                let target = self
                    .program
                    .declarations
                    .get(call.function)
                    .expect("resolved direct-call target must exist");
                if arguments.len() == call.arguments.len()
                    && call.arguments.len() == target.parameters.len()
                {
                    for (argument, parameter) in arguments.iter().zip(&target.parameters) {
                        valid &= require_type(
                            argument.ty,
                            lower_type(&parameter.type_syntax),
                            argument.span,
                            "call argument",
                            self.diagnostics,
                        );
                    }
                } else if call.arguments.len() != target.parameters.len() {
                    self.diagnostics.push(
                        Diagnostic::error(
                            WRONG_ARGUMENT_COUNT,
                            format!(
                                "function `{}` expects {} argument{} but received {}",
                                target.name,
                                target.parameters.len(),
                                if target.parameters.len() == 1 {
                                    ""
                                } else {
                                    "s"
                                },
                                call.arguments.len()
                            ),
                        )
                        .with_primary_label(
                            call.callee_span,
                            "called with the wrong number of arguments",
                        )
                        .with_secondary_label(target.name_span, "function declared here"),
                    );
                    valid = false;
                }

                if !valid {
                    return None;
                }
                Some(HirExpression {
                    kind: HirExpressionKind::DirectCall {
                        function: call.function,
                        arguments,
                    },
                    ty: lower_type(&target.return_type),
                    span: call.span,
                })
            }
            ResolvedExpression::Grouped(grouped) => {
                let inner = self.check_expression(&grouped.expression)?;
                let ty = inner.ty;
                Some(HirExpression {
                    kind: HirExpressionKind::Grouped(Box::new(inner)),
                    ty,
                    span: grouped.span,
                })
            }
        }
    }

    fn binding_type(&self, binding: BindingId) -> Type {
        assert_eq!(
            binding.function(),
            self.declaration.id,
            "resolved binding must belong to the current function"
        );
        match binding {
            BindingId::Parameter(id) => lower_type(
                &self
                    .declaration
                    .parameter(id)
                    .expect("resolved parameter ID must exist")
                    .type_syntax,
            ),
            BindingId::Local(id) => lower_type(
                &self
                    .definition
                    .local(id)
                    .expect("resolved local ID must exist")
                    .type_syntax,
            ),
        }
    }
}

fn select_binary_operation(
    operator: ResolvedBinaryOperator,
    operand_type: Type,
) -> Option<HirBinaryOperation> {
    match (operator, operand_type) {
        (ResolvedBinaryOperator::Add, Type::I64) => Some(HirBinaryOperation::AddI64),
        (ResolvedBinaryOperator::Subtract, Type::I64) => Some(HirBinaryOperation::SubtractI64),
        (ResolvedBinaryOperator::Multiply, Type::I64) => Some(HirBinaryOperation::MultiplyI64),
        (ResolvedBinaryOperator::Add, Type::U64) => Some(HirBinaryOperation::AddU64),
        (ResolvedBinaryOperator::Subtract, Type::U64) => Some(HirBinaryOperation::SubtractU64),
        (ResolvedBinaryOperator::Multiply, Type::U64) => Some(HirBinaryOperation::MultiplyU64),
        (ResolvedBinaryOperator::Add, Type::U8) => Some(HirBinaryOperation::AddU8),
        (ResolvedBinaryOperator::Subtract, Type::U8) => Some(HirBinaryOperation::SubtractU8),
        (ResolvedBinaryOperator::Multiply, Type::U8) => Some(HirBinaryOperation::MultiplyU8),
        (ResolvedBinaryOperator::Add, Type::F64) => Some(HirBinaryOperation::AddF64),
        (ResolvedBinaryOperator::Subtract, Type::F64) => Some(HirBinaryOperation::SubtractF64),
        (ResolvedBinaryOperator::Multiply, Type::F64) => Some(HirBinaryOperation::MultiplyF64),
        (_, Type::Bool | Type::Unit) => None,
    }
}

pub(super) fn require_type(
    actual: Type,
    expected: Type,
    span: Span,
    context: &'static str,
    diagnostics: &mut Diagnostics,
) -> bool {
    if actual == expected {
        return true;
    }
    diagnostics.push(
        Diagnostic::error(
            TYPE_MISMATCH,
            format!(
                "{context} has type `{}` but `{}` is required",
                actual.name(),
                expected.name()
            ),
        )
        .with_primary_label(span, "type mismatch"),
    );
    false
}

pub(super) fn is_direct_call_through_groups(expression: &ResolvedExpression) -> bool {
    match expression {
        ResolvedExpression::DirectCall(_) => true,
        ResolvedExpression::Grouped(grouped) => is_direct_call_through_groups(&grouped.expression),
        _ => false,
    }
}
