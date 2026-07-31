//! Binding, literal, operator, grouping, and excluded-construction expressions.

use super::*;
use crate::{
    diagnostics::format_type_list,
    hir::{
        HirBinaryOperation, HirComparisonOperand, HirComparisonPredicate, HirExpressionKind,
        HirIntegerCast, HirIntegerType, HirPrimitiveComparison, HirUnaryOperation,
    },
    resolve::{ResolvedBinaryOperator, ResolvedIntegerType, ResolvedUnaryOperator},
};

use crate::typeck::{
    literal::{classify_i64_magnitude, i64_literal_through_groups, Magnitude},
    program::{INVALID_CONSTRUCTION, INVALID_OBJECT_CONTEXT},
};

const NUMERIC_TYPE_NAMES: &[&str] = &["i64", "u64", "u8", "f64"];
const INTEGER_TYPE_NAMES: &[&str] = &["i64", "u64", "u8"];
const EQUALITY_TYPE_NAMES: &[&str] = &["i64", "u64", "u8", "f64", "bool"];
const ORDERING_TYPE_NAMES: &[&str] = &["i64", "u64", "u8", "f64"];
const NEGATABLE_TYPE_NAMES: &[&str] = &["i64", "f64"];

impl CallableChecker<'_, '_> {
    pub(super) fn check_integer_cast(
        &mut self,
        cast: &crate::resolve::ResolvedIntegerCastExpr,
    ) -> Option<HirExpression> {
        let operand = self.check_expression(&cast.source)?;
        let Some(source) = HirIntegerType::from_type(operand.ty) else {
            self.diagnostics.push(
                Diagnostic::error(
                    TYPE_MISMATCH,
                    "integer cast requires a primitive integer source",
                )
                .with_primary_label(
                    operand.span,
                    format!("source has type `{}`", operand.ty.name()),
                )
                .with_secondary_label(cast.target_span, "primitive integer cast target")
                .with_note(format!(
                    "integer source types are {}",
                    format_type_list(INTEGER_TYPE_NAMES)
                )),
            );
            return None;
        };
        let target = match cast.target {
            ResolvedIntegerType::I64 => HirIntegerType::I64,
            ResolvedIntegerType::U64 => HirIntegerType::U64,
            ResolvedIntegerType::U8 => HirIntegerType::U8,
        };
        let operation = HirIntegerCast { source, target };
        Some(HirExpression {
            kind: HirExpressionKind::IntegerCast {
                operation,
                operand: Box::new(operand),
            },
            ty: operation.result_type(),
            span: cast.span,
        })
    }

    pub(super) fn check_binding_expression(
        &mut self,
        binding: &crate::resolve::ResolvedBindingExpr,
    ) -> Option<HirExpression> {
        let ty = self.binding_type(binding.binding);
        if matches!(ty, Type::Class(_) | Type::Obj | Type::Interface(_)) {
            let message = if matches!(ty, Type::Obj | Type::Interface(_)) {
                "an object view cannot be used as an ordinary value"
            } else {
                "an inline object cannot be used as an ordinary value"
            };
            self.diagnostics.push(
                Diagnostic::error(INVALID_OBJECT_CONTEXT, message).with_primary_label(
                    binding.span,
                    "use the object as a field or method receiver",
                ),
            );
            return None;
        }
        Some(HirExpression {
            kind: HirExpressionKind::Binding(binding.binding),
            ty,
            span: binding.span,
        })
    }

    pub(super) fn check_boolean_expression(
        &mut self,
        boolean: &crate::resolve::ResolvedBooleanExpr,
    ) -> Option<HirExpression> {
        Some(HirExpression {
            kind: HirExpressionKind::Boolean(boolean.value),
            ty: Type::Bool,
            span: boolean.span,
        })
    }

    pub(super) fn check_unary_expression(
        &mut self,
        unary: &crate::resolve::ResolvedUnaryExpr,
    ) -> Option<HirExpression> {
        if unary.operator == ResolvedUnaryOperator::BitwiseComplement {
            return self.check_bitwise_complement(unary);
        }
        match unary.operator {
            ResolvedUnaryOperator::Negate => {
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
            ResolvedUnaryOperator::LogicalNot => {
                let actual = self.static_expression_type(&unary.operand);
                if actual != Type::Bool {
                    self.diagnostics.push(
                        Diagnostic::error(
                            TYPE_MISMATCH,
                            "logical negation requires a `bool` operand",
                        )
                        .with_primary_label(
                            unary.operator_span,
                            "operator cannot be applied to this operand",
                        )
                        .with_secondary_label(
                            unary.operand.span(),
                            format!("operand has type `{}`", actual.name()),
                        ),
                    );
                    return None;
                }
            }
            ResolvedUnaryOperator::BitwiseComplement => unreachable!("returned above"),
        }

        let operand = self.check_expression(&unary.operand)?;
        let operation = match unary.operator {
            ResolvedUnaryOperator::LogicalNot => {
                debug_assert_eq!(operand.ty, Type::Bool);
                HirUnaryOperation::LogicalNotBool
            }
            ResolvedUnaryOperator::BitwiseComplement => unreachable!("returned above"),
            ResolvedUnaryOperator::Negate => match operand.ty {
                Type::I64 => HirUnaryOperation::NegateI64,
                Type::F64 => HirUnaryOperation::NegateF64,
                _ => {
                    self.diagnostics.push(
                        Diagnostic::error(
                            TYPE_MISMATCH,
                            format!(
                                "unary negation requires an {} operand",
                                format_type_list(NEGATABLE_TYPE_NAMES)
                            ),
                        )
                        .with_primary_label(
                            operand.span,
                            format!("operand has type `{}`", operand.ty.name()),
                        ),
                    );
                    return None;
                }
            },
        };
        let ty = operation.result_type();
        Some(HirExpression {
            kind: HirExpressionKind::Unary {
                operation,
                operand: Box::new(operand),
            },
            ty,
            span: unary.span,
        })
    }

    pub(super) fn check_binary_expression(
        &mut self,
        binary: &crate::resolve::ResolvedBinaryExpr,
    ) -> Option<HirExpression> {
        if let Some(predicate) = comparison_predicate(binary.operator) {
            return self.check_primitive_comparison(binary, predicate);
        }
        if matches!(
            binary.operator,
            ResolvedBinaryOperator::ShiftLeft | ResolvedBinaryOperator::ShiftRight
        ) {
            return self.check_shift_expression(binary);
        }
        if matches!(
            binary.operator,
            ResolvedBinaryOperator::BitwiseAnd
                | ResolvedBinaryOperator::BitwiseOr
                | ResolvedBinaryOperator::BitwiseXor
        ) {
            return self.check_integer_bitwise_expression(binary);
        }
        if matches!(
            binary.operator,
            ResolvedBinaryOperator::Divide | ResolvedBinaryOperator::Remainder
        ) {
            return self.check_division_expression(binary);
        }

        // Both operands are checked in source order so independent diagnostics accumulate.
        let left = self.check_expression(&binary.left);
        let right = self.check_expression(&binary.right);
        let (left, right) = match (left, right) {
            (Some(left), Some(right)) => (left, right),
            _ => return None,
        };
        let operation = (left.ty == right.ty)
            .then(|| select_arithmetic_operation(binary.operator, left.ty))
            .flatten();
        let Some(operation) = operation else {
            self.diagnostics.push(
                Diagnostic::error(
                    TYPE_MISMATCH,
                    "binary arithmetic requires operands of the same numeric type",
                )
                .with_primary_label(
                    binary.operator_span,
                    "operator cannot be applied to these operand types",
                )
                .with_secondary_label(
                    left.span,
                    format!("left operand has type `{}`", left.ty.name()),
                )
                .with_secondary_label(
                    right.span,
                    format!("right operand has type `{}`", right.ty.name()),
                )
                .with_note(format!(
                    "numeric operand types are {}",
                    format_type_list(NUMERIC_TYPE_NAMES)
                )),
            );
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

    fn check_primitive_comparison(
        &mut self,
        binary: &crate::resolve::ResolvedBinaryExpr,
        predicate: HirComparisonPredicate,
    ) -> Option<HirExpression> {
        let left_type = self.static_expression_type(&binary.left);
        let right_type = self.static_expression_type(&binary.right);
        let operand = comparison_operand(predicate, left_type, right_type);
        let Some(operand) = operand else {
            let equality = matches!(
                predicate,
                HirComparisonPredicate::Equal | HirComparisonPredicate::NotEqual
            );
            let spelling = comparison_spelling(predicate);
            let (message, type_description, type_names) = if equality {
                (
                    format!(
                        "binary `{spelling}` requires operands of the same supported primitive type"
                    ),
                    "equality operand types",
                    EQUALITY_TYPE_NAMES,
                )
            } else {
                (
                    format!("binary `{spelling}` requires operands of the same numeric type"),
                    "numeric operand types",
                    ORDERING_TYPE_NAMES,
                )
            };
            self.diagnostics.push(
                Diagnostic::error(TYPE_MISMATCH, message)
                    .with_primary_label(
                        binary.operator_span,
                        "operator cannot be applied to these operand types",
                    )
                    .with_secondary_label(
                        binary.left.span(),
                        format!("left operand has type `{}`", left_type.name()),
                    )
                    .with_secondary_label(
                        binary.right.span(),
                        format!("right operand has type `{}`", right_type.name()),
                    )
                    .with_note(format!(
                        "{type_description} are {}",
                        format_type_list(type_names)
                    )),
            );
            return None;
        };

        // Valid comparison operands are checked exactly once in source order.
        let left = self.check_expression(&binary.left);
        let right = self.check_expression(&binary.right);
        let (left, right) = match (left, right) {
            (Some(left), Some(right)) => (left, right),
            _ => return None,
        };
        let operation = HirPrimitiveComparison { predicate, operand };
        Some(HirExpression {
            kind: HirExpressionKind::PrimitiveComparison {
                operation,
                left: Box::new(left),
                right: Box::new(right),
            },
            ty: operation.result_type(),
            span: binary.span,
        })
    }

    pub(super) fn check_grouped_expression(
        &mut self,
        grouped: &crate::resolve::ResolvedGroupedExpr,
    ) -> Option<HirExpression> {
        let inner = self.check_expression(&grouped.expression)?;
        let ty = inner.ty;
        Some(HirExpression {
            kind: HirExpressionKind::Grouped(Box::new(inner)),
            ty,
            span: grouped.span,
        })
    }

    pub(super) fn check_excluded_construction_expression(
        &mut self,
        construction: &crate::resolve::ResolvedConstructExpr,
    ) -> Option<HirExpression> {
        match &construction.mode {
            crate::resolve::ResolvedConstructionMode::Initialize { arguments } => {
                for argument in arguments {
                    let _ = self.check_expression(argument);
                }
            }
            crate::resolve::ResolvedConstructionMode::Copy { source, .. } => {
                let _ = self.check_expression(source);
            }
        }
        self.diagnostics.push(
            Diagnostic::error(
                INVALID_CONSTRUCTION,
                "construction is not allowed in this expression context",
            )
            .with_primary_label(
                construction.span,
                "use this object source in initialization, assignment, an object argument, or an object return",
            ),
        );
        None
    }
}

fn comparison_operand(
    predicate: HirComparisonPredicate,
    left: Type,
    right: Type,
) -> Option<HirComparisonOperand> {
    if left != right {
        return None;
    }
    if let Some(integer) = HirIntegerType::from_type(left) {
        return Some(HirComparisonOperand::Integer(integer));
    }
    if left == Type::F64 {
        return Some(HirComparisonOperand::F64);
    }
    (left == Type::Bool
        && matches!(
            predicate,
            HirComparisonPredicate::Equal | HirComparisonPredicate::NotEqual
        ))
    .then_some(HirComparisonOperand::Bool)
}

const fn comparison_spelling(predicate: HirComparisonPredicate) -> &'static str {
    match predicate {
        HirComparisonPredicate::Equal => "==",
        HirComparisonPredicate::NotEqual => "!=",
        HirComparisonPredicate::LessThan => "<",
        HirComparisonPredicate::LessEqual => "<=",
        HirComparisonPredicate::GreaterThan => ">",
        HirComparisonPredicate::GreaterEqual => ">=",
    }
}

const fn comparison_predicate(operator: ResolvedBinaryOperator) -> Option<HirComparisonPredicate> {
    match operator {
        ResolvedBinaryOperator::Equal => Some(HirComparisonPredicate::Equal),
        ResolvedBinaryOperator::NotEqual => Some(HirComparisonPredicate::NotEqual),
        ResolvedBinaryOperator::LessThan => Some(HirComparisonPredicate::LessThan),
        ResolvedBinaryOperator::LessEqual => Some(HirComparisonPredicate::LessEqual),
        ResolvedBinaryOperator::GreaterThan => Some(HirComparisonPredicate::GreaterThan),
        ResolvedBinaryOperator::GreaterEqual => Some(HirComparisonPredicate::GreaterEqual),
        ResolvedBinaryOperator::Add
        | ResolvedBinaryOperator::Subtract
        | ResolvedBinaryOperator::Multiply
        | ResolvedBinaryOperator::Divide
        | ResolvedBinaryOperator::Remainder
        | ResolvedBinaryOperator::ShiftLeft
        | ResolvedBinaryOperator::ShiftRight
        | ResolvedBinaryOperator::BitwiseAnd
        | ResolvedBinaryOperator::BitwiseOr
        | ResolvedBinaryOperator::BitwiseXor => None,
    }
}

fn select_arithmetic_operation(
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
        (
            ResolvedBinaryOperator::Equal
            | ResolvedBinaryOperator::NotEqual
            | ResolvedBinaryOperator::LessThan
            | ResolvedBinaryOperator::LessEqual
            | ResolvedBinaryOperator::GreaterThan
            | ResolvedBinaryOperator::GreaterEqual,
            _,
        ) => None,
        (
            ResolvedBinaryOperator::BitwiseAnd
            | ResolvedBinaryOperator::BitwiseOr
            | ResolvedBinaryOperator::BitwiseXor
            | ResolvedBinaryOperator::ShiftLeft
            | ResolvedBinaryOperator::ShiftRight
            | ResolvedBinaryOperator::Divide
            | ResolvedBinaryOperator::Remainder,
            _,
        ) => None,
        (
            _,
            Type::Bool
            | Type::Unit
            | Type::Obj
            | Type::Class(_)
            | Type::Interface(_)
            | Type::Shared(_)
            | Type::OptionalShared(_)
            | Type::OptionalPrimitive(_)
            | Type::OptionalClass(_)
            | Type::Array(_),
        ) => None,
    }
}
