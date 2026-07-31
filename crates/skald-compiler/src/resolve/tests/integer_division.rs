use super::*;

fn expect_binary(
    expression: &ResolvedExpression,
    operator: ResolvedBinaryOperator,
) -> &ResolvedBinaryExpr {
    let ResolvedExpression::Binary(binary) = expression else {
        panic!("expected resolved binary expression");
    };
    assert_eq!(binary.operator, operator);
    binary
}

#[test]
fn resolution_preserves_division_identity_shape_spans_and_dumps() {
    let source = concat!(
        "fn combine(left: i64, first: i64, second: i64) -> i64 {\n",
        "  return left / first % second;\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    );
    let output = resolve_text(source);
    assert!(!output.has_errors());
    let definition = output.program.definitions.get(FunctionId::new(0)).unwrap();
    let expression = return_value(&definition.body.statements[0]);
    let remainder = expect_binary(expression, ResolvedBinaryOperator::Remainder);
    let division = expect_binary(&remainder.left, ResolvedBinaryOperator::Divide);
    assert_eq!(&source[division.operator_span.range().as_range()], "/");
    assert_eq!(&source[remainder.operator_span.range().as_range()], "%");

    let dump = dump_resolved(&output.program);
    assert_eq!(dump, dump_resolved(&output.program));
    assert!(dump.contains("Binary Divide"));
    assert!(dump.contains("Binary Remainder"));
}
