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
fn resolution_preserves_shift_identity_shape_spans_and_dumps() {
    let source = concat!(
        "fn combine(left: u64, first: u64, second: u64) -> u64 {\n",
        "  return left << first >> second;\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    );
    let output = resolve_text(source);
    assert!(!output.has_errors());
    let definition = output.program.definitions.get(FunctionId::new(0)).unwrap();
    let expression = return_value(&definition.body.statements[0]);
    let right = expect_binary(expression, ResolvedBinaryOperator::ShiftRight);
    let left = expect_binary(&right.left, ResolvedBinaryOperator::ShiftLeft);
    assert_eq!(&source[left.operator_span.range().as_range()], "<<");
    assert_eq!(&source[right.operator_span.range().as_range()], ">>");

    let dump = dump_resolved(&output.program);
    assert_eq!(dump, dump_resolved(&output.program));
    assert!(dump.contains("Binary ShiftLeft"));
    assert!(dump.contains("Binary ShiftRight"));
}
