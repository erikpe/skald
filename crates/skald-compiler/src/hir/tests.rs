use crate::{
    identity::{FunctionId, LoopId},
    test_support::type_check_source,
};

use super::{
    dump_hir, HirBlock, HirComparisonOperand, HirControlEffects, HirExpression, HirExpressionKind,
    HirFunctionDefinition, HirPrimitiveComparison, HirReturnValue, HirStatement, HirUnaryOperation,
    HirWhile, Type,
};

fn returned_expression_mut(definition: &mut HirFunctionDefinition) -> &mut HirExpression {
    let HirStatement::Return(statement) = definition.body.statements.last_mut().unwrap() else {
        panic!("expected final return statement");
    };
    let HirReturnValue::Scalar(expression) =
        statement.value.as_mut().expect("expected return value")
    else {
        panic!("expected scalar return value");
    };
    expression
}

#[test]
fn dumps_manually_constructed_structured_while_deterministically() {
    let mut hir = type_check_source("fn main() -> i64 { return 0; }\n")
        .hir
        .unwrap();
    let entry = hir.entry_function;
    let definition = hir.definitions.get_mut_for_test(entry).unwrap();
    let span = definition.body.span;
    let loop_id = LoopId::new(entry, 0);
    let body = HirBlock {
        statements: vec![],
        effects: HirControlEffects::fallthrough(),
        span,
    };
    definition.body.statements.insert(
        0,
        HirStatement::While(HirWhile::new(
            loop_id,
            HirExpression {
                kind: HirExpressionKind::Boolean(true),
                ty: Type::Bool,
                span,
            },
            body,
            span,
        )),
    );

    let dump = dump_hir(&hir);
    let lines: Vec<_> = dump
        .lines()
        .filter(|line| {
            line.contains("While ")
                || line.trim_start().starts_with("Condition ")
                || line.trim_start().starts_with("Boolean ")
        })
        .map(|line| line.split(" @").next().unwrap().trim())
        .collect();
    assert_eq!(
        lines,
        ["While f0:loop0", "Condition", "Boolean true : bool",]
    );
}

#[test]
fn dumps_manually_constructed_eager_boolean_operations_deterministically() {
    let mut hir = type_check_source(concat!(
        "fn invert() -> bool { return false; }\n",
        "fn compare() -> bool { return 1 == 2; }\n",
        "fn main() -> i64 { return 0; }\n",
    ))
    .hir
    .unwrap();

    let invert = returned_expression_mut(
        hir.definitions
            .get_mut_for_test(FunctionId::new(0))
            .unwrap(),
    );
    let span = invert.span;
    invert.kind = HirExpressionKind::Unary {
        operation: HirUnaryOperation::LogicalNotBool,
        operand: Box::new(HirExpression {
            kind: HirExpressionKind::Boolean(false),
            ty: Type::Bool,
            span,
        }),
    };
    invert.ty = Type::Bool;

    let comparison = returned_expression_mut(
        hir.definitions
            .get_mut_for_test(FunctionId::new(1))
            .unwrap(),
    );
    let HirExpressionKind::PrimitiveComparison {
        operation,
        left,
        right,
    } = &mut comparison.kind
    else {
        panic!("expected comparison expression");
    };
    operation.operand = HirComparisonOperand::Bool;
    left.kind = HirExpressionKind::Boolean(true);
    left.ty = Type::Bool;
    right.kind = HirExpressionKind::Boolean(false);
    right.ty = Type::Bool;

    assert_eq!(HirUnaryOperation::LogicalNotBool.operand_type(), Type::Bool);
    assert_eq!(HirUnaryOperation::LogicalNotBool.result_type(), Type::Bool);
    assert!(HirPrimitiveComparison {
        predicate: operation.predicate,
        operand: HirComparisonOperand::Bool,
    }
    .is_valid());

    let dump = dump_hir(&hir);
    assert_eq!(dump, dump_hir(&hir));
    assert!(dump.contains("Unary LogicalNotBool : bool"));
    assert!(dump.contains("BooleanComparison eq.bool : bool"));
    assert!(!dump.contains("BooleanComparison lt.bool"));
}
