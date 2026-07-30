use super::*;

#[test]
fn resolution_preserves_logical_operator_identity_grouping_and_precedence() {
    let output = resolve_text(concat!(
        "fn evaluate(a: bool, b: bool, c: bool) -> bool { return (a || b) && !c; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(!output.has_errors());
    let definition = output.program.definitions.get(FunctionId::new(0)).unwrap();
    let ResolvedExpression::Logical(and) = return_value(&definition.body.statements[0]) else {
        panic!("expected resolved logical and");
    };
    assert_eq!(and.operator, ResolvedLogicalOperator::And);
    let ResolvedExpression::Grouped(grouped) = and.left.as_ref() else {
        panic!("expected resolved grouping");
    };
    assert!(matches!(
        grouped.expression.as_ref(),
        ResolvedExpression::Logical(ResolvedLogicalExpr {
            operator: ResolvedLogicalOperator::Or,
            ..
        })
    ));
    assert!(matches!(
        and.right.as_ref(),
        ResolvedExpression::Unary(ResolvedUnaryExpr {
            operator: ResolvedUnaryOperator::LogicalNot,
            ..
        })
    ));

    let dump = dump_resolved(&output.program);
    assert_eq!(dump, dump_resolved(&output.program));
    assert!(dump.contains("Logical And"));
    assert!(dump.contains("Logical Or"));
}
