//! Exact primitive comparison selection and diagnostics.

use super::*;
use crate::{
    diagnostics::format_type_list,
    hir::{
        HirComparisonOperand, HirComparisonPredicate, HirExpressionKind, HirIntegerType,
        HirPrimitiveComparison,
    },
    resolve::ResolvedBinaryOperator,
};

const EQUALITY_TYPE_NAMES: &[&str] = &["i64", "u64", "u8", "f64", "bool"];
const ORDERING_TYPE_NAMES: &[&str] = &["i64", "u64", "u8", "f64"];

impl CallableChecker<'_, '_> {
    pub(super) fn check_primitive_comparison(
        &mut self,
        binary: &crate::resolve::ResolvedBinaryExpr,
        predicate: HirComparisonPredicate,
    ) -> Option<HirExpression> {
        let left_type = self.static_expression_type(&binary.left);
        let right_type = self.static_expression_type(&binary.right);
        let Some(operand) = comparison_operand(predicate, left_type, right_type) else {
            self.report_invalid_comparison(binary, predicate, left_type, right_type);
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

    fn report_invalid_comparison(
        &mut self,
        binary: &crate::resolve::ResolvedBinaryExpr,
        predicate: HirComparisonPredicate,
        left_type: Type,
        right_type: Type,
    ) {
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

pub(super) const fn comparison_predicate(
    operator: ResolvedBinaryOperator,
) -> Option<HirComparisonPredicate> {
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
