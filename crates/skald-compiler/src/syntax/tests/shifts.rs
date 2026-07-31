use super::*;

fn expect_binary(expression: &Expression, operator: BinaryOperator) -> &BinaryExpr {
    let Expression::Binary(binary) = expression else {
        panic!("expected binary expression");
    };
    assert_eq!(binary.operator, operator);
    binary
}

#[test]
fn shift_precedence_is_between_additive_and_bitwise_tiers() {
    let source = concat!(
        "fn combine(a: u64, b: u64, c: u64, d: u64, e: u64, f: u64, g: u64, h: u64, flag: bool, other: bool) -> bool {\n",
        "  return a + b << c + d & e ^ f | g == h && flag || other;\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    );
    let (_, output) = parse_text(source);
    assert!(!output.has_errors());

    let Expression::Logical(or) = return_value(function(&output.ast, 0)) else {
        panic!("expected outer logical OR");
    };
    let Expression::Logical(and) = or.left.as_ref() else {
        panic!("expected logical AND inside OR");
    };
    let comparison = expect_binary(&and.left, BinaryOperator::Equal);
    let bitwise_or = expect_binary(&comparison.left, BinaryOperator::BitwiseOr);
    let bitwise_xor = expect_binary(&bitwise_or.left, BinaryOperator::BitwiseXor);
    let bitwise_and = expect_binary(&bitwise_xor.left, BinaryOperator::BitwiseAnd);
    let shift = expect_binary(&bitwise_and.left, BinaryOperator::ShiftLeft);
    assert_eq!(
        expect_binary(&shift.left, BinaryOperator::Add).operator,
        BinaryOperator::Add
    );
    assert_eq!(
        expect_binary(&shift.right, BinaryOperator::Add).operator,
        BinaryOperator::Add
    );

    let dump = dump_ast(&output.ast);
    assert_eq!(dump, dump_ast(&output.ast));
    assert!(dump.contains("Binary ShiftLeft"));
}

#[test]
fn shift_chains_are_left_associative_and_grouping_is_explicit() {
    let source = concat!(
        "fn left(a: u64, b: u64, c: u64) -> u64 { return a << b >> c; }\n",
        "fn grouped(a: u64, b: u64, c: u64) -> u64 { return a << (b >> c); }\n",
        "fn main() -> i64 { return 0; }\n",
    );
    let (_, output) = parse_text(source);
    assert!(!output.has_errors());

    let outer = expect_binary(
        return_value(function(&output.ast, 0)),
        BinaryOperator::ShiftRight,
    );
    let inner = expect_binary(&outer.left, BinaryOperator::ShiftLeft);
    assert_eq!(&source[inner.operator_span.range().as_range()], "<<");
    assert_eq!(&source[outer.operator_span.range().as_range()], ">>");

    let grouped = expect_binary(
        return_value(function(&output.ast, 1)),
        BinaryOperator::ShiftLeft,
    );
    let Expression::Grouped(right) = grouped.right.as_ref() else {
        panic!("expected explicit grouped right shift");
    };
    expect_binary(&right.expression, BinaryOperator::ShiftRight);
}

#[test]
fn missing_shift_operands_recover_at_following_statements() {
    for spelling in ["1 <<", "1 >>"] {
        let source = format!("fn main() -> i64 {{ var invalid: i64 = {spelling}; return 0; }}");
        let (_, output) = parse_text(&source);
        assert!(output.has_errors(), "{source}");
        assert!(matches!(
            function(&output.ast, 0).body.statements.last(),
            Some(Statement::Return(_))
        ));
    }
}
