use super::*;

#[test]
fn logical_tiers_preserve_frozen_precedence_and_associativity() {
    let (_, output) = parse_text(concat!(
        "fn precedence(a: bool, b: bool, c: bool) -> bool { return a || b && c; }\n",
        "fn comparison(a: bool, b: bool, c: bool) -> bool { return a && b == c; }\n",
        "fn presence(a: bool?, b: bool) -> bool { return a is some || b; }\n",
        "fn chain(a: bool, b: bool, c: bool) -> bool { return a && b && c; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(!output.has_errors());

    let Expression::Logical(or) = return_value(function(&output.ast, 0)) else {
        panic!("expected logical or");
    };
    assert_eq!(or.operator, LogicalOperator::Or);
    assert!(matches!(
        or.right.as_ref(),
        Expression::Logical(LogicalExpr {
            operator: LogicalOperator::And,
            ..
        })
    ));

    let Expression::Logical(and) = return_value(function(&output.ast, 1)) else {
        panic!("expected logical and");
    };
    assert!(matches!(
        and.right.as_ref(),
        Expression::Binary(BinaryExpr {
            operator: BinaryOperator::Equal,
            ..
        })
    ));

    let Expression::Logical(or) = return_value(function(&output.ast, 2)) else {
        panic!("expected logical or");
    };
    assert!(matches!(
        or.left.as_ref(),
        Expression::PresenceTest(PresenceTestExpr {
            kind: PresenceTestKind::Some,
            ..
        })
    ));

    let Expression::Logical(outer) = return_value(function(&output.ast, 3)) else {
        panic!("expected outer logical and");
    };
    assert!(matches!(
        outer.left.as_ref(),
        Expression::Logical(LogicalExpr {
            operator: LogicalOperator::And,
            ..
        })
    ));
}

#[test]
fn grouping_and_prefix_and_postfix_not_remain_explicit() {
    let (sources, output) = parse_text(concat!(
        "fn grouped(a: bool, b: bool, c: bool?) -> bool { return !a && (b || c!); }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(!output.has_errors());
    let Expression::Logical(and) = return_value(function(&output.ast, 0)) else {
        panic!("expected logical and");
    };
    assert!(matches!(
        and.left.as_ref(),
        Expression::Unary(UnaryExpr {
            operator: UnaryOperator::LogicalNot,
            ..
        })
    ));
    let Expression::Grouped(grouped) = and.right.as_ref() else {
        panic!("expected explicit grouping");
    };
    let Expression::Logical(or) = grouped.expression.as_ref() else {
        panic!("expected grouped logical or");
    };
    assert!(matches!(or.right.as_ref(), Expression::Unwrap(_)));
    let source = sources.get(and.operator_span.source_id()).unwrap();
    assert_eq!(source.slice(and.operator_span.range()), Some("&&"));
    assert_eq!(source.slice(or.operator_span.range()), Some("||"));

    let dump = dump_ast(&output.ast);
    assert_eq!(dump, dump_ast(&output.ast));
    assert!(dump.contains("Logical And"));
    assert!(dump.contains("Logical Or"));
    assert!(dump.contains("Unary LogicalNot"));
    assert!(dump.contains("Unwrap"));
}

#[test]
fn long_ungrouped_logical_chains_do_not_consume_nesting_budget() {
    let chain = std::iter::repeat_n("true", MAX_SYNTAX_NESTING * 3)
        .collect::<Vec<_>>()
        .join(" && ");
    let source =
        format!("fn chain() -> bool {{ return {chain}; }} fn main() -> i64 {{ return 0; }}");
    let (_, output) = parse_text(&source);

    assert!(!output.has_errors());
}

#[test]
fn missing_logical_right_operands_recover_at_following_statements() {
    for operator in ["&&", "||"] {
        let source = format!(
            "fn invalid() -> bool {{ var value: bool = true {operator}; return false; }} \
             fn main() -> i64 {{ return 0; }}"
        );
        let (_, output) = parse_text(&source);
        let diagnostic = output
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == EXPECTED_EXPRESSION)
            .unwrap();
        assert_eq!(
            diagnostic.message,
            format!("expected a right operand after `{operator}`")
        );
        assert!(matches!(
            function(&output.ast, 0).body.statements.last(),
            Some(Statement::Return(_))
        ));
    }
}

#[test]
fn longer_malformed_logical_runs_recover_without_hiding_later_statements() {
    for malformed in ["&&&", "|||"] {
        let mut sources = SourceDatabase::new();
        let source_id = sources.add(
            "test.ska",
            format!(
                "fn invalid() -> bool {{ var value: bool = true {malformed} false; return false; }} \
                 fn main() -> i64 {{ return 0; }}"
            ),
        );
        let source = sources.get(source_id).unwrap();
        let lexed = lex(source);
        assert_eq!(lexed.diagnostics.len(), 1);
        let output = parse(source, &lexed.tokens);
        assert!(output
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == EXPECTED_EXPRESSION));
        assert!(matches!(
            function(&output.ast, 0).body.statements.last(),
            Some(Statement::Return(_))
        ));
    }
}
