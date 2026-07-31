use super::*;

fn expect_binary(expression: &Expression, operator: BinaryOperator) -> &BinaryExpr {
    let Expression::Binary(binary) = expression else {
        panic!("expected binary expression");
    };
    assert_eq!(binary.operator, operator);
    binary
}

#[test]
fn bitwise_precedence_is_between_additive_comparison_and_logical_tiers() {
    let (_, output) = parse_text(concat!(
        "fn combine(a: u64, b: u64, c: u64, d: u64, e: u64, f: u64, g: bool, h: bool) -> bool {\n",
        "  return ~a + b & c ^ d | e == f && g || h;\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(!output.has_errors());

    let Expression::Logical(or) = return_value(function(&output.ast, 0)) else {
        panic!("expected outer logical OR");
    };
    assert_eq!(or.operator, LogicalOperator::Or);
    let Expression::Logical(and) = or.left.as_ref() else {
        panic!("expected logical AND inside OR");
    };
    assert_eq!(and.operator, LogicalOperator::And);
    let comparison = expect_binary(&and.left, BinaryOperator::Equal);
    let bitwise_or = expect_binary(&comparison.left, BinaryOperator::BitwiseOr);
    let bitwise_xor = expect_binary(&bitwise_or.left, BinaryOperator::BitwiseXor);
    let bitwise_and = expect_binary(&bitwise_xor.left, BinaryOperator::BitwiseAnd);
    let additive = expect_binary(&bitwise_and.left, BinaryOperator::Add);
    assert!(matches!(
        additive.left.as_ref(),
        Expression::Unary(UnaryExpr {
            operator: UnaryOperator::BitwiseComplement,
            ..
        })
    ));

    let dump = dump_ast(&output.ast);
    assert_eq!(dump, dump_ast(&output.ast));
    for operation in [
        "Unary BitwiseComplement",
        "Binary BitwiseAnd",
        "Binary BitwiseXor",
        "Binary BitwiseOr",
    ] {
        assert!(dump.contains(operation));
    }
}

#[test]
fn bitwise_prefix_is_right_associative_and_each_binary_tier_is_left_associative() {
    let (_, output) = parse_text(concat!(
        "fn prefix(value: u8?) -> u8 { return ~~~value!; }\n",
        "fn and_chain(a: u8, b: u8, c: u8) -> u8 { return a & b & c; }\n",
        "fn xor_chain(a: u8, b: u8, c: u8) -> u8 { return a ^ b ^ c; }\n",
        "fn or_chain(a: u8, b: u8, c: u8) -> u8 { return a | b | c; }\n",
        "fn casts(value: u8) -> u8 { return ~(u8) ~value; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(!output.has_errors());

    let mut prefix = return_value(function(&output.ast, 0));
    for _ in 0..3 {
        let Expression::Unary(unary) = prefix else {
            panic!("expected nested bitwise complement");
        };
        assert_eq!(unary.operator, UnaryOperator::BitwiseComplement);
        prefix = &unary.operand;
    }
    assert!(matches!(prefix, Expression::Unwrap(_)));

    for (function_index, operation) in [
        (1, BinaryOperator::BitwiseAnd),
        (2, BinaryOperator::BitwiseXor),
        (3, BinaryOperator::BitwiseOr),
    ] {
        let outer = expect_binary(
            return_value(function(&output.ast, function_index)),
            operation,
        );
        assert_eq!(expect_binary(&outer.left, operation).operator, operation);
    }

    let Expression::Unary(outer) = return_value(function(&output.ast, 4)) else {
        panic!("expected complement around cast");
    };
    assert!(matches!(
        outer.operand.as_ref(),
        Expression::PrimitiveCast(_)
    ));
}

#[test]
fn missing_bitwise_operands_recover_at_following_statements() {
    for spelling in ["~", "1 &", "1 ^", "1 |"] {
        let source = format!("fn main() -> i64 {{ var invalid: i64 = {spelling}; return 0; }}");
        let (_, output) = parse_text(&source);
        assert!(output.has_errors(), "{source}");
        assert!(matches!(
            function(&output.ast, 0).body.statements.last(),
            Some(Statement::Return(_))
        ));
    }
}
