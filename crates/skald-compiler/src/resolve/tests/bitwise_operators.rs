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
fn resolution_preserves_bitwise_identity_shape_spans_and_dumps() {
    let source = concat!(
        "fn combine(left: u64, right: u64) -> u64 {\n",
        "  return ~left & right ^ left | right;\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    );
    let output = resolve_text(source);
    assert!(!output.has_errors());
    let definition = output.program.definitions.get(FunctionId::new(0)).unwrap();
    let expression = return_value(&definition.body.statements[0]);
    let bitwise_or = expect_binary(expression, ResolvedBinaryOperator::BitwiseOr);
    let bitwise_xor = expect_binary(&bitwise_or.left, ResolvedBinaryOperator::BitwiseXor);
    let bitwise_and = expect_binary(&bitwise_xor.left, ResolvedBinaryOperator::BitwiseAnd);
    let ResolvedExpression::Unary(complement) = bitwise_and.left.as_ref() else {
        panic!("expected resolved bitwise complement");
    };
    assert_eq!(
        complement.operator,
        ResolvedUnaryOperator::BitwiseComplement
    );
    assert_eq!(&source[complement.operator_span.range().as_range()], "~");
    assert_eq!(&source[bitwise_and.operator_span.range().as_range()], "&");
    assert_eq!(&source[bitwise_xor.operator_span.range().as_range()], "^");
    assert_eq!(&source[bitwise_or.operator_span.range().as_range()], "|");

    let dump = dump_resolved(&output.program);
    assert_eq!(dump, dump_resolved(&output.program));
    for operation in [
        "Unary BitwiseComplement",
        "Binary BitwiseAnd",
        "Binary BitwiseXor",
        "Binary BitwiseOr",
    ] {
        assert!(dump.contains(operation));
    }
}
