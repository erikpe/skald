use super::*;
use crate::{
    hir::{
        dump_hir, HirBinaryOperation, HirExpression, HirExpressionKind, HirFunctionDefinition,
        HirLocalInitializer, HirReturnValue, HirStatement, Type,
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
        HirExpressionKind::Binary { left, right, .. }
        | HirExpressionKind::PrimitiveComparison { left, right, .. } => {
            assert_expression_is_fully_typed(left);
            assert_expression_is_fully_typed(right);
        }
        HirExpressionKind::Logical(_) => {
            panic!("i64 typing helper does not accept boolean logical expressions")
        }
        HirExpressionKind::IntegerCast { operation, operand } => {
            assert_eq!(operand.ty, operation.source_type());
            assert_eq!(expression.ty, operation.result_type());
        }
        HirExpressionKind::DirectCall { arguments, .. }
        | HirExpressionKind::StaticCall { arguments, .. } => {
            for argument in arguments {
                assert_call_argument_is_fully_typed(argument);
            }
        }
        HirExpressionKind::MethodCall { arguments, .. } => {
            for argument in arguments {
                assert_call_argument_is_fully_typed(argument);
            }
        }
        HirExpressionKind::InterfaceCall { arguments, .. } => {
            for argument in arguments {
                assert_call_argument_is_fully_typed(argument);
            }
        }
        HirExpressionKind::FieldRead(_)
        | HirExpressionKind::TypeTest(_)
        | HirExpressionKind::PresenceTest { .. }
        | HirExpressionKind::Unwrap(_) => {}
        HirExpressionKind::Binding(_)
        | HirExpressionKind::I64(_)
        | HirExpressionKind::U64(_)
        | HirExpressionKind::U8(_)
        | HirExpressionKind::F64Bits(_)
        | HirExpressionKind::Boolean(_) => {}
        HirExpressionKind::ArrayConstruction(_)
        | HirExpressionKind::ArrayLength(_)
        | HirExpressionKind::ArrayElement(_)
        | HirExpressionKind::ArraySlice(_) => {
            panic!("scalar typing helper does not accept array expressions")
        }
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
        crate::hir::HirCallArgument::View(view) => {
            assert!(matches!(
                view.access,
                crate::hir::HirAccess::ReadOnly | crate::hir::HirAccess::Mutable
            ));
        }
        crate::hir::HirCallArgument::CheckedView(view) => {
            assert!(matches!(
                view.consumer_access,
                crate::hir::HirAccess::ReadOnly | crate::hir::HirAccess::Mutable
            ));
        }
        crate::hir::HirCallArgument::Copy(_) => {}
        crate::hir::HirCallArgument::Shared(_) => {}
        crate::hir::HirCallArgument::Optional { .. } => {}
        crate::hir::HirCallArgument::ClassOptional(_) => {}
        crate::hir::HirCallArgument::OptionalShared(_) => {}
        crate::hir::HirCallArgument::OptionalPlace(_) => {}
        crate::hir::HirCallArgument::Array(_) => {}
        crate::hir::HirCallArgument::ArrayAlias(_) => {}
    }
}

fn class_alias_view(
    argument: &crate::hir::HirCallArgument,
) -> (&crate::hir::HirObjectView, &crate::hir::HirObjectPlace) {
    let crate::hir::HirCallArgument::View(view) = argument else {
        panic!("expected class alias view argument");
    };
    let crate::hir::HirViewSource::Place(place) = &view.source else {
        panic!("expected class alias view to retain its static place");
    };
    (view, place)
}

fn source_place(source: &crate::hir::HirObjectSource) -> &crate::hir::HirObjectPlace {
    let crate::hir::HirObjectSource::Place(place) = source else {
        panic!("expected an existing-place object source");
    };
    place
}

mod alias_parameters;
mod arrays;
mod bitwise_operators;
mod capabilities;
mod comparisons;
mod control_flow;
mod declarations;
mod destructors;
mod diagnostics;
mod dumps;
mod eager_boolean_operators;
mod expressions;
mod inline_fields;
mod integer_casts;
mod interfaces;
mod literals;
mod object_results;
mod objects;
mod optional_values;
mod primitive_binding_assignment;
mod shared_ownership;
mod short_circuit_boolean;
mod static_methods;
mod type_operations;
mod value_parameters;
mod while_loops;
