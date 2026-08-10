//! Exact floating division and checked integer division/remainder selection.

use super::*;
use crate::{
    diagnostics::format_type_list,
    hir::{
        HirBinaryOperation, HirCheckedIntegerDivision, HirExpressionKind, HirIntegerDivisionKind,
        HirIntegerDivisionOperation, HirIntegerType,
    },
    resolve::{ResolvedBinaryExpr, ResolvedBinaryOperator},
};

const NUMERIC_TYPE_NAMES: &[&str] = &["i64", "u64", "u8", "f64"];
const INTEGER_TYPE_NAMES: &[&str] = &["i64", "u64", "u8"];

#[derive(Clone, Copy)]
enum DivisionSelection {
    Floating,
    Integer(HirIntegerDivisionOperation),
}

impl CallableChecker<'_, '_> {
    pub(super) fn check_division_expression(
        &mut self,
        binary: &ResolvedBinaryExpr,
    ) -> Option<HirExpression> {
        let left_type = self.static_expression_type(&binary.left);
        let right_type = self.static_expression_type(&binary.right);
        let selection = select_division(binary.operator, left_type, right_type);
        let Some(selection) = selection else {
            self.report_invalid_division(binary, left_type, right_type);
            return None;
        };

        // Valid operands are checked exactly once in source order. Floating
        // division remains a pure binary rvalue; only the integer selection
        // carries checked zero-divisor control flow.
        let left = self.check_expression(&binary.left);
        let right = self.check_expression(&binary.right);
        let (left, right) = match (left, right) {
            (Some(left), Some(right)) => (left, right),
            _ => return None,
        };

        Some(match selection {
            DivisionSelection::Floating => HirExpression {
                kind: HirExpressionKind::Binary {
                    operation: HirBinaryOperation::DivideF64,
                    left: Box::new(left),
                    right: Box::new(right),
                },
                ty: Type::F64,
                span: binary.span,
            },
            DivisionSelection::Integer(operation) => HirExpression {
                kind: HirExpressionKind::CheckedIntegerDivision(Box::new(
                    HirCheckedIntegerDivision::new(operation, left, right),
                )),
                ty: operation.result_type(),
                span: binary.span,
            },
        })
    }

    fn report_invalid_division(
        &mut self,
        binary: &ResolvedBinaryExpr,
        left_type: Type,
        right_type: Type,
    ) {
        let (message, type_description, type_names) = match binary.operator {
            ResolvedBinaryOperator::Divide => (
                "binary `/` requires operands of the same numeric type",
                "numeric operand types",
                NUMERIC_TYPE_NAMES,
            ),
            ResolvedBinaryOperator::Remainder => (
                "integer `%` requires operands of the same primitive integer type",
                "integer operand types",
                INTEGER_TYPE_NAMES,
            ),
            _ => unreachable!("division checker must receive `/` or `%`"),
        };
        self.diagnostics.push(
            Diagnostic::error(TYPE_MISMATCH, message)
                .with_primary_label(
                    binary.operator_span,
                    "operator cannot be applied to these operand types",
                )
                .with_secondary_label(
                    binary.left.span(),
                    format!(
                        "left operand has type `{}`",
                        self.diagnostic_type_name(left_type)
                    ),
                )
                .with_secondary_label(
                    binary.right.span(),
                    format!(
                        "right operand has type `{}`",
                        self.diagnostic_type_name(right_type)
                    ),
                )
                .with_note(format!(
                    "{type_description} are {}",
                    format_type_list(type_names)
                )),
        );
    }
}

fn select_division(
    operator: ResolvedBinaryOperator,
    left: Type,
    right: Type,
) -> Option<DivisionSelection> {
    if left != right {
        return None;
    }
    if operator == ResolvedBinaryOperator::Divide && left == Type::F64 {
        return Some(DivisionSelection::Floating);
    }
    let operand = HirIntegerType::from_type(left)?;
    let kind = match operator {
        ResolvedBinaryOperator::Divide => HirIntegerDivisionKind::Quotient,
        ResolvedBinaryOperator::Remainder => HirIntegerDivisionKind::Remainder,
        _ => return None,
    };
    Some(DivisionSelection::Integer(HirIntegerDivisionOperation {
        kind,
        operand,
    }))
}
