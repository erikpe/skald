use super::*;

fn expect_binary(expression: &Expression, operator: BinaryOperator) -> &BinaryExpr {
    let Expression::Binary(binary) = expression else {
        panic!("expected binary expression");
    };
    assert_eq!(binary.operator, operator);
    binary
}

#[test]
fn multiplicative_family_is_left_associative_with_frozen_precedence() {
    let source = concat!(
        "fn combine(a: i64, b: i64, c: i64, d: i64, e: i64, count: u64, mask: i64, flag: bool) -> bool {\n",
        "  return -a * b / c % d + e << count & mask == 0 && flag || false;\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    );
    let (_, output) = parse_text(source);
    assert!(!output.has_errors());

    let Expression::Logical(or) = return_value(function(&output.ast, 0)) else {
        panic!("expected logical OR");
    };
    let Expression::Logical(and) = or.left.as_ref() else {
        panic!("expected logical AND");
    };
    let comparison = expect_binary(&and.left, BinaryOperator::Equal);
    let bitwise = expect_binary(&comparison.left, BinaryOperator::BitwiseAnd);
    let shift = expect_binary(&bitwise.left, BinaryOperator::ShiftLeft);
    let additive = expect_binary(&shift.left, BinaryOperator::Add);
    let remainder = expect_binary(&additive.left, BinaryOperator::Remainder);
    let division = expect_binary(&remainder.left, BinaryOperator::Divide);
    let multiplication = expect_binary(&division.left, BinaryOperator::Multiply);
    assert!(matches!(multiplication.left.as_ref(), Expression::Unary(_)));
    assert_eq!(&source[division.operator_span.range().as_range()], "/");
    assert_eq!(&source[remainder.operator_span.range().as_range()], "%");

    let dump = dump_ast(&output.ast);
    assert_eq!(dump, dump_ast(&output.ast));
    assert!(dump.contains("Binary Divide"));
    assert!(dump.contains("Binary Remainder"));
}

#[test]
fn grouping_can_override_multiplicative_left_association() {
    let (_, output) = parse_text(concat!(
        "fn plain(a: i64, b: i64, c: i64) -> i64 { return a / b % c; }\n",
        "fn grouped(a: i64, b: i64, c: i64) -> i64 { return a / (b % c); }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(!output.has_errors());
    let plain = expect_binary(
        return_value(function(&output.ast, 0)),
        BinaryOperator::Remainder,
    );
    expect_binary(&plain.left, BinaryOperator::Divide);
    let grouped = expect_binary(
        return_value(function(&output.ast, 1)),
        BinaryOperator::Divide,
    );
    let Expression::Grouped(right) = grouped.right.as_ref() else {
        panic!("expected grouped remainder");
    };
    expect_binary(&right.expression, BinaryOperator::Remainder);
}

#[test]
fn division_binds_before_the_specialized_is_tier() {
    let (_, output) = parse_text(concat!(
        "fn inspect(a: i64, b: i64, flag: bool) -> bool {\n",
        "  return a / b is some && flag;\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(!output.has_errors());
    let Expression::Logical(logical) = return_value(function(&output.ast, 0)) else {
        panic!("expected logical expression");
    };
    let Expression::PresenceTest(test) = logical.left.as_ref() else {
        panic!("expected presence test");
    };
    expect_binary(&test.source, BinaryOperator::Divide);
}

#[test]
fn comments_remain_valid_at_the_end_of_division_expressions() {
    let (_, output) = parse_text(concat!(
        "fn divide() -> i64 {\n",
        "  var quotient: i64 = 8 / 2; // / and % are comment text\n",
        "  return quotient % 3; // adjacent comment boundary\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(!output.has_errors());
    expect_binary(
        return_value(function(&output.ast, 0)),
        BinaryOperator::Remainder,
    );
}

#[test]
fn malformed_multiplicative_operators_recover_at_following_statements() {
    for expression in ["1 /", "1 %", "1 / / 2", "1 %% 2"] {
        let source = format!(
            "fn invalid() -> i64 {{ var value: i64 = {expression}; return 7; }} \
             fn main() -> i64 {{ return 0; }}"
        );
        let (_, first) = parse_text(&source);
        let (_, second) = parse_text(&source);
        assert!(first.has_errors(), "{source}");
        assert_eq!(
            first
                .diagnostics
                .iter()
                .map(|diagnostic| (diagnostic.code, diagnostic.message.as_str()))
                .collect::<Vec<_>>(),
            second
                .diagnostics
                .iter()
                .map(|diagnostic| (diagnostic.code, diagnostic.message.as_str()))
                .collect::<Vec<_>>()
        );
        assert!(matches!(
            function(&first.ast, 0).body.statements.last(),
            Some(Statement::Return(_))
        ));
    }
}
