use super::*;

#[test]
fn range_is_lowest_precedence_and_preserves_all_spans() {
    let (sources, output) = parse_text("fn value() -> i64 { return 1 + 2 .. 3 || false; }");
    assert!(!output.has_errors());
    let Expression::Range(range) = return_value(function(&output.ast, 0)) else {
        panic!("expected a range expression");
    };
    assert!(matches!(*range.lower, Expression::Binary(_)));
    assert!(matches!(*range.upper, Expression::Logical(_)));
    let source = sources.get(range.span.source_id()).unwrap();
    assert_eq!(source.slice(range.operator_span.range()), Some(".."));
    assert_eq!(
        source.slice(range.span.range()),
        Some("1 + 2 .. 3 || false")
    );

    let dump = dump_ast(&output.ast);
    assert!(dump.contains("Range"));
    assert!(dump.contains("DotDot"));
    assert_eq!(dump, dump_ast(&output.ast));
}

#[test]
fn grouping_allows_nested_ranges_but_ungrouped_chains_are_rejected_once() {
    let (_, grouped) = parse_text("fn value() -> i64 { return (1 .. 2) .. (3 .. 4); }");
    assert!(!grouped.has_errors());
    let Expression::Range(outer) = return_value(function(&grouped.ast, 0)) else {
        panic!("expected outer range");
    };
    assert!(matches!(*outer.lower, Expression::Grouped(_)));
    assert!(matches!(*outer.upper, Expression::Grouped(_)));

    let (_, chained) = parse_text(concat!(
        "fn value() -> i64 { return 1 .. 2 .. 3; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert_eq!(
        chained
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == INVALID_RANGE_EXPRESSION)
            .count(),
        1
    );
    assert_eq!(chained.ast.declarations.len(), 2);
}

#[test]
fn missing_range_endpoints_recover_at_statement_and_argument_boundaries() {
    let (_, output) = parse_text(concat!(
        "fn take(value: i64) -> unit {}\n",
        "fn broken() -> i64 {\n",
        "  var lower: i64 = .. 2;\n",
        "  take(1 ..);\n",
        "  return 0;\n",
        "}\n",
    ));
    assert_eq!(
        output
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == INVALID_RANGE_EXPRESSION)
            .count(),
        2
    );
    assert!(function(&output.ast, 1)
        .body
        .statements
        .iter()
        .any(|statement| matches!(statement, Statement::Return(_))));
}

#[test]
fn missing_endpoints_recover_across_return_expression_and_for_header_boundaries() {
    let (_, output) = parse_text(concat!(
        "fn broken_return() -> i64 { return 1 ..; }\n",
        "fn broken_loop() -> i64 { for (item in 1 ..) {} return 0; }\n",
        "fn recovered() -> i64 { return 0; }\n",
    ));
    assert_eq!(
        output
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == INVALID_RANGE_EXPRESSION)
            .count(),
        2,
        "{:?}",
        output.diagnostics,
    );
    assert!(output.ast.declarations.iter().any(|declaration| {
        matches!(declaration, TopLevelDeclaration::Function(function) if function.name.text == "recovered")
    }));
}
