use super::*;

const OPERATORS: &[(BinaryOperator, &str)] = &[
    (BinaryOperator::Equal, "=="),
    (BinaryOperator::NotEqual, "!="),
    (BinaryOperator::LessThan, "<"),
    (BinaryOperator::LessEqual, "<="),
    (BinaryOperator::GreaterThan, ">"),
    (BinaryOperator::GreaterEqual, ">="),
];

#[test]
fn parses_every_comparison_operator_with_exact_source_shape() {
    for (expected, spelling) in OPERATORS {
        let source = format!("fn main() -> i64 {{ return left {spelling} right; }}");
        let (sources, output) = parse_text(&source);
        assert!(!output.has_errors(), "{spelling}");
        let Expression::Binary(comparison) = return_value(function(&output.ast, 0)) else {
            panic!("expected comparison for {spelling}");
        };

        assert_eq!(comparison.operator, *expected, "{spelling}");
        assert_eq!(
            sources
                .get(comparison.operator_span.source_id())
                .unwrap()
                .slice(comparison.operator_span.range()),
            Some(*spelling)
        );
    }
}

#[test]
fn comparisons_follow_arithmetic_and_precede_contextual_is() {
    let (_, output) = parse_text("fn main() -> i64 { return left + 1 < right * 2 is some; }");
    assert!(!output.has_errors());
    let Expression::PresenceTest(test) = return_value(function(&output.ast, 0)) else {
        panic!("contextual `is` must be the weakest expression form");
    };
    let Expression::Binary(comparison) = test.source.as_ref() else {
        panic!("presence source must be the comparison");
    };
    assert_eq!(comparison.operator, BinaryOperator::LessThan);
    assert!(matches!(
        comparison.left.as_ref(),
        Expression::Binary(BinaryExpr {
            operator: BinaryOperator::Add,
            ..
        })
    ));
    assert!(matches!(
        comparison.right.as_ref(),
        Expression::Binary(BinaryExpr {
            operator: BinaryOperator::Multiply,
            ..
        })
    ));
}

#[test]
fn grouping_preserves_nested_comparisons_and_stable_dumps() {
    let (_, output) = parse_text("fn main() -> i64 { return (left < right); }");
    assert!(!output.has_errors());
    let Expression::Grouped(grouped) = return_value(function(&output.ast, 0)) else {
        panic!("expected grouping");
    };
    assert!(matches!(
        grouped.expression.as_ref(),
        Expression::Binary(BinaryExpr {
            operator: BinaryOperator::LessThan,
            ..
        })
    ));

    let dump = dump_ast(&output.ast);
    assert_eq!(dump, dump_ast(&output.ast));
    assert!(dump.contains("Binary LessThan"));
}

#[test]
fn rejects_a_complete_ungrouped_chain_and_recovers_at_the_next_statement() {
    let (_, output) = parse_text(concat!(
        "fn main() -> i64 {\n",
        "  var invalid: bool = first < second <= third > fourth;\n",
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
    let main = function(&output.ast, 0);
    assert!(matches!(
        main.body.statements.last(),
        Some(Statement::Return(_))
    ));
}
