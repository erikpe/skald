use super::*;
use crate::{
    hir::{
        dump_hir, BlockFlow, HirBinaryOperation, HirExpression, HirExpressionKind,
        HirFunctionDefinition, HirLocalInitializer, HirReturnValue, HirStatement, Type,
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
    let HirReturnValue::Scalar(value) = statement.value.as_ref().expect("expected a return value")
    else {
        panic!("expected a scalar return value");
    };
    value
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
                assert_call_argument_is_fully_typed(argument);
            }
        }
        HirExpressionKind::MethodCall { arguments, .. } => {
            for argument in arguments {
                assert_call_argument_is_fully_typed(argument);
            }
        }
        HirExpressionKind::FieldRead(_) => {}
        HirExpressionKind::Binding(_)
        | HirExpressionKind::I64(_)
        | HirExpressionKind::U64(_)
        | HirExpressionKind::U8(_)
        | HirExpressionKind::F64Bits(_)
        | HirExpressionKind::Boolean(_) => {}
    }
}

fn assert_call_argument_is_fully_typed(argument: &crate::hir::HirCallArgument) {
    match argument {
        crate::hir::HirCallArgument::Value(expression) => {
            assert_expression_is_fully_typed(expression)
        }
        crate::hir::HirCallArgument::Place(place) => {
            assert!(matches!(
                place.access,
                crate::hir::HirAccess::ReadOnly | crate::hir::HirAccess::Mutable
            ));
        }
        crate::hir::HirCallArgument::Copy(copy) => {
            if let crate::hir::HirObjectSource::Place(place) = &copy.source {
                assert!(matches!(
                    place.access,
                    crate::hir::HirAccess::ReadOnly | crate::hir::HirAccess::Mutable
                ));
            }
        }
    }
}

fn source_place(source: &crate::hir::HirObjectSource) -> &crate::hir::HirObjectPlace {
    let crate::hir::HirObjectSource::Place(place) = source else {
        panic!("expected an existing-place object source");
    };
    place
}

mod alias_parameters;
mod capabilities;
mod control_flow;
mod declarations;
mod destructors;
mod diagnostics;
mod dumps;
mod expressions;
mod inline_fields;
mod literals;
mod object_results;
mod objects;
mod value_parameters;
