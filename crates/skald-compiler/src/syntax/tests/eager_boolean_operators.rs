use super::*;

#[test]
fn prefix_logical_not_is_right_associative_and_binds_outside_postfix_unwrap() {
    let (sources, output) = parse_text(concat!(
        "fn nested(flag: bool) -> bool { return !!flag; }\n",
        "fn optional(flag: bool?) -> bool { return !flag!; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(!output.has_errors());

    let Expression::Unary(outer) = return_value(function(&output.ast, 0)) else {
        panic!("expected outer logical negation");
    };
    assert_eq!(outer.operator, UnaryOperator::LogicalNot);
    assert!(matches!(
        outer.operand.as_ref(),
        Expression::Unary(UnaryExpr {
            operator: UnaryOperator::LogicalNot,
            ..
        })
    ));

    let Expression::Unary(logical_not) = return_value(function(&output.ast, 1)) else {
        panic!("expected logical negation around optional unwrap");
    };
    assert_eq!(logical_not.operator, UnaryOperator::LogicalNot);
    let source = sources.get(logical_not.span.source_id()).unwrap();
    assert_eq!(source.slice(logical_not.operator_span.range()), Some("!"));
    assert_eq!(source.slice(logical_not.span.range()), Some("!flag!"));
    assert!(matches!(
        logical_not.operand.as_ref(),
        Expression::Unwrap(_)
    ));

    let dump = dump_ast(&output.ast);
    assert_eq!(dump, dump_ast(&output.ast));
    assert!(dump.contains("Unary LogicalNot"));
    assert!(dump.contains("Unwrap"));
}

#[test]
fn logical_not_participates_in_comparison_and_cast_precedence() {
    let (_, output) = parse_text(concat!(
        "fn left(flag: bool, expected: bool) -> bool { return !flag == expected; }\n",
        "fn right(flag: bool, expected: bool) -> bool { return expected == !flag; }\n",
        "fn grouped(flag: bool, expected: bool) -> bool { return !(flag == expected); }\n",
        "fn cast_shape(flag: bool) -> i64 { return (i64) !flag; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(!output.has_errors());

    let Expression::Binary(left) = return_value(function(&output.ast, 0)) else {
        panic!("expected comparison");
    };
    assert!(matches!(
        left.left.as_ref(),
        Expression::Unary(UnaryExpr {
            operator: UnaryOperator::LogicalNot,
            ..
        })
    ));

    let Expression::Binary(right) = return_value(function(&output.ast, 1)) else {
        panic!("expected comparison");
    };
    assert!(matches!(
        right.right.as_ref(),
        Expression::Unary(UnaryExpr {
            operator: UnaryOperator::LogicalNot,
            ..
        })
    ));

    let Expression::Unary(grouped) = return_value(function(&output.ast, 2)) else {
        panic!("expected logical negation");
    };
    assert!(matches!(grouped.operand.as_ref(), Expression::Grouped(_)));

    let Expression::IntegerCast(cast) = return_value(function(&output.ast, 3)) else {
        panic!("expected integer cast");
    };
    assert!(matches!(
        cast.source.as_ref(),
        Expression::Unary(UnaryExpr {
            operator: UnaryOperator::LogicalNot,
            ..
        })
    ));
}

#[test]
fn comparison_with_logical_not_precedes_contextual_is() {
    let (_, output) = parse_text(concat!(
        "fn inspect(flag: bool, expected: bool) -> bool {\n",
        "  return !flag == expected is some;\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(!output.has_errors());

    let Expression::PresenceTest(test) = return_value(function(&output.ast, 0)) else {
        panic!("contextual `is` must remain outside the comparison");
    };
    let Expression::Binary(comparison) = test.source.as_ref() else {
        panic!("presence source must be the comparison");
    };
    assert_eq!(comparison.operator, BinaryOperator::Equal);
    assert!(matches!(
        comparison.left.as_ref(),
        Expression::Unary(UnaryExpr {
            operator: UnaryOperator::LogicalNot,
            ..
        })
    ));
}

#[test]
fn grouped_postfix_unwrap_is_not_reinterpreted_as_an_object_cast() {
    let (_, output) = parse_text(concat!(
        "fn unwrap(flag: bool?) -> bool { return (flag)!; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(!output.has_errors());

    let Expression::Unwrap(unwrap) = return_value(function(&output.ast, 0)) else {
        panic!("expected postfix unwrap");
    };
    assert!(matches!(unwrap.source.as_ref(), Expression::Grouped(_)));
}

#[test]
fn logical_not_does_not_make_comparison_chains_associative() {
    let (_, output) = parse_text(concat!(
        "fn main() -> i64 {\n",
        "  var invalid: bool = !first == second != !third;\n",
        "  return 0;\n",
        "}\n",
    ));

    let diagnostics: Vec<_> = output
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == INVALID_COMPARISON)
        .collect();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0].message,
        "comparison operators cannot be chained"
    );
    assert!(matches!(
        function(&output.ast, 0).body.statements.last(),
        Some(Statement::Return(_))
    ));
}

#[test]
fn missing_logical_not_operand_recovers_at_the_next_statement() {
    let (_, output) = parse_text(concat!(
        "fn main() -> i64 {\n",
        "  var invalid: bool = !;\n",
        "  return 0;\n",
        "}\n",
    ));

    assert!(output.has_errors());
    assert!(matches!(
        function(&output.ast, 0).body.statements.last(),
        Some(Statement::Return(_))
    ));
}
