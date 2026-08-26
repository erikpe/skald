use super::*;
use crate::{
    hir::{
        dump_hir, HirBinaryOperation, HirExpression, HirExpressionKind, HirFunctionDefinition,
        HirLocalInitializer, HirReturnValue, HirStatement, Type,
    },
    identity::FunctionId,
    test_support::{
        resolve_generic_source, resolve_source, type_check_generic_source, type_check_source,
    },
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

fn check_generic_source(text: &str) -> crate::hir::HirProgram {
    let checked = type_check_generic_source(text);
    assert!(
        checked.diagnostics.is_empty(),
        "generic source must type check: {:?}",
        checked.diagnostics
    );
    checked.hir.expect("valid generic source must produce HIR")
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
        HirExpressionKind::CheckedShift(_) => {
            panic!("i64 typing helper does not accept checked shifts")
        }
        HirExpressionKind::CheckedIntegerDivision(division) => {
            assert_expression_is_fully_typed(&division.dividend);
            assert_expression_is_fully_typed(&division.divisor);
        }
        HirExpressionKind::PrimitiveCast { operation, operand } => {
            assert_eq!(operand.ty, operation.source_type());
            assert_eq!(expression.ty, operation.result_type());
        }
        HirExpressionKind::Io(_) => {}
        HirExpressionKind::DirectCall { arguments, .. }
        | HirExpressionKind::StaticCall { arguments, .. } => {
            for argument in arguments {
                assert_call_argument_is_fully_typed(argument);
            }
        }
        HirExpressionKind::IndirectCall(call) => {
            assert_eq!(call.callee.ty, Type::Function(call.function_type));
            assert_eq!(call.result, expression.ty);
            assert_eq!(call.span, expression.span);
            for argument in &call.arguments {
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
        | HirExpressionKind::StaticRead(_)
        | HirExpressionKind::TypeTest(_)
        | HirExpressionKind::PresenceTest { .. }
        | HirExpressionKind::OptionalBoxPresence(_)
        | HirExpressionKind::Unwrap(_)
        | HirExpressionKind::NestedOptionalUnwrap(_) => {}
        HirExpressionKind::Binding(_)
        | HirExpressionKind::FunctionReference(_)
        | HirExpressionKind::I64(_)
        | HirExpressionKind::U64(_)
        | HirExpressionKind::U8(_)
        | HirExpressionKind::F64Bits(_)
        | HirExpressionKind::Boolean(_) => {}
        HirExpressionKind::ArrayConstruction(_)
        | HirExpressionKind::OptionalArrayUnwrap(_)
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
        crate::hir::HirCallArgument::SharedPlace(_) => {}
        crate::hir::HirCallArgument::Optional { .. } => {}
        crate::hir::HirCallArgument::ClassOptional(_) => {}
        crate::hir::HirCallArgument::OptionalShared(_) => {}
        crate::hir::HirCallArgument::AggregateOptional(_) => {}
        crate::hir::HirCallArgument::OptionalPlace(_) => {}
        crate::hir::HirCallArgument::OptionalSharedPlace(_) => {}
        crate::hir::HirCallArgument::Array(_) => {}
        crate::hir::HirCallArgument::ArrayAlias(_) => {}
        crate::hir::HirCallArgument::PrimitivePlace(_) => {}
        crate::hir::HirCallArgument::ProducedPrimitiveAlias(expression) => {
            assert!(matches!(
                expression.ty,
                Type::I64 | Type::U64 | Type::U8 | Type::F64 | Type::Bool
            ));
        }
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

fn receiver_place(receiver: &crate::hir::HirObjectReceiver) -> &crate::hir::HirObjectPlace {
    receiver
        .inspection_place()
        .expect("test receiver must retain an inspectable object place")
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
mod final_fields;
mod floating_division;
mod function_values;
mod generic_classes;
mod generic_interfaces;
mod generic_object_model;
mod indirect_calls;
mod inline_fields;
mod integer_division;
mod interfaces;
mod iteration;
mod literals;
mod object_results;
mod objects;
mod optional_values;
mod primitive_binding_assignment;
mod primitive_casts;
mod private_cell_fields;
mod produced_fields;
mod produced_receivers;
mod receiver_carriers;
mod shared_optional_boxes;
mod shared_ownership;
mod shifts;
mod short_circuit_boolean;
mod static_fields;
mod static_methods;
mod structural_indexing;
mod type_operations;
mod value_parameters;
mod while_loops;
