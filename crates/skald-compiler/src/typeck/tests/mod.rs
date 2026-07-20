use super::*;
use crate::{
    hir::{
        dump_hir, BlockFlow, HirBinaryOperation, HirExpression, HirExpressionKind,
        HirFunctionDefinition, HirStatement, Type,
    },
    identity::FunctionId,
    test_support::{resolve_source, type_check_source},
};

fn check_text(text: &str) -> TypeCheckOutput {
    type_check_source(text)
}

fn resolve_text(text: &str) -> crate::resolve::ResolvedProgram {
    let resolved = resolve_source(text);
    assert!(
        resolved.diagnostics.is_empty(),
        "test source must resolve cleanly"
    );
    resolved.program
}

fn returned_expression(function: &HirFunctionDefinition) -> &HirExpression {
    let HirStatement::Return(statement) = function.body.statements.last().unwrap() else {
        panic!("expected final return statement");
    };
    statement.value.as_ref().expect("expected a return value")
}

fn assert_expression_is_fully_typed(expression: &HirExpression) {
    assert_eq!(expression.ty, Type::I64);
    match &expression.kind {
        HirExpressionKind::Unary { operand, .. } | HirExpressionKind::Grouped(operand) => {
            assert_expression_is_fully_typed(operand)
        }
        HirExpressionKind::Binary { left, right, .. } => {
            assert_expression_is_fully_typed(left);
            assert_expression_is_fully_typed(right);
        }
        HirExpressionKind::DirectCall { arguments, .. } => {
            for argument in arguments {
                assert_expression_is_fully_typed(argument);
            }
        }
        HirExpressionKind::Binding(_)
        | HirExpressionKind::I64(_)
        | HirExpressionKind::U64(_)
        | HirExpressionKind::U8(_)
        | HirExpressionKind::F64Bits(_)
        | HirExpressionKind::Boolean(_) => {}
    }
}

mod control_flow;
mod declarations;
mod diagnostics;
mod dumps;
mod expressions;
mod literals;
