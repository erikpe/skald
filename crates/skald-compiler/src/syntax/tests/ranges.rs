use super::*;
use crate::diagnostics::LabelStyle;

fn first_for_in(output: &ParseOutput) -> &ForInStatement {
    let Statement::ForIn(statement) = &function(&output.ast, 0).body.statements[0] else {
        panic!("expected a for-in statement");
    };
    statement
}

#[test]
fn direct_range_source_is_distinct_and_preserves_endpoint_precedence_and_spans() {
    let (sources, output) =
        parse_text("fn main() -> unit { for (item in 1 + 2 .. 3 || false) {} }");
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let ForInSource::Range(range) = &first_for_in(&output).source else {
        panic!("expected a direct range source");
    };
    assert!(matches!(range.lower, Expression::Binary(_)));
    assert!(matches!(range.upper, Expression::Logical(_)));
    let source = sources.get(range.span.source_id()).unwrap();
    assert_eq!(source.slice(range.operator_span.range()), Some(".."));
    assert_eq!(
        source.slice(range.span.range()),
        Some("1 + 2 .. 3 || false")
    );

    let dump = dump_ast(&output.ast);
    assert!(dump.contains("RangeSource"));
    assert!(dump.contains("DotDot"));
    assert_eq!(dump, dump_ast(&output.ast));
}

#[test]
fn grouped_endpoints_and_parenthesized_iterables_are_valid_but_grouped_ranges_are_not() {
    let (_, valid) = parse_text(concat!(
        "fn endpoints() -> unit { for (item in (1 + 2) .. (4)) {} }\n",
        "fn iterable(values: Values) -> unit { for (item in (values)) {} }\n",
    ));
    assert!(!valid.has_errors(), "{:?}", valid.diagnostics);
    let ForInSource::Range(range) = &first_for_in(&valid).source else {
        panic!("expected a direct range source");
    };
    assert!(matches!(range.lower, Expression::Grouped(_)));
    assert!(matches!(range.upper, Expression::Grouped(_)));
    let Statement::ForIn(iterable) = &function(&valid.ast, 1).body.statements[0] else {
        panic!("expected an ordinary for-in statement");
    };
    assert!(matches!(
        iterable.source,
        ForInSource::Iterable(Expression::Grouped(_))
    ));

    let (_, grouped_range) = parse_text(concat!(
        "fn broken() -> i64 { for (item in (1 .. 3)) {} return 0; }\n",
        "fn recovered() -> i64 { return 0; }\n",
    ));
    assert_eq!(
        grouped_range
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == INVALID_RANGE_SYNTAX)
            .count(),
        1,
        "{:?}",
        grouped_range.diagnostics,
    );
    assert!(grouped_range.ast.declarations.iter().any(|declaration| {
        matches!(declaration, TopLevelDeclaration::Function(function) if function.name.text == "recovered")
    }));
}

#[test]
fn range_syntax_is_rejected_in_every_general_expression_context() {
    for source in [
        "fn broken() -> unit { var value: Range<u64> = 1u .. 3u; }",
        "fn broken(value: u64) -> unit { value = 1u .. 3u; }",
        "fn broken() -> unit { consume(1u .. 3u); }",
        "fn broken() -> Range<u64> { return 1u .. 3u; }",
        "fn broken() -> unit { var value: u64 = choose(1u .. 3u); }",
    ] {
        let (_, output) = parse_text(source);
        let diagnostic = output
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == INVALID_RANGE_SYNTAX)
            .unwrap_or_else(|| panic!("missing direct-range diagnostic: {source}"));
        assert_eq!(
            diagnostic.message,
            "concise range syntax is allowed only as the direct `for-in` source"
        );
        assert_eq!(diagnostic.labels[0].style, LabelStyle::Primary);
        assert_eq!(
            diagnostic.labels[0].message,
            "use an explicit `Range<T>(lower, upper)` value here"
        );
    }
}

#[test]
fn malformed_direct_ranges_report_once_and_recover_at_for_header_boundaries() {
    for source in [
        "fn broken() -> i64 { for (item in .. 3u) {} return 0; }",
        "fn broken() -> i64 { for (item in 1u ..) {} return 0; }",
        "fn broken() -> i64 { for (item in 1u .. 2u .. 3u) {} return 0; }",
    ] {
        let (_, output) = parse_text(source);
        assert_eq!(
            output
                .diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.code == INVALID_RANGE_SYNTAX)
                .count(),
            1,
            "{source}: {:?}",
            output.diagnostics,
        );
        assert!(function(&output.ast, 0)
            .body
            .statements
            .iter()
            .any(|statement| matches!(statement, Statement::Return(_))));
    }
}

#[test]
fn rejected_value_ranges_recover_into_later_statements_and_declarations() {
    let (_, output) = parse_text(concat!(
        "fn take(value: Range<u64>) -> unit {}\n",
        "fn broken() -> i64 {\n",
        "  var local: Range<u64> = 1u .. 2u;\n",
        "  take(2u .. 3u);\n",
        "  return 3u .. 4u;\n",
        "  return 0;\n",
        "}\n",
        "fn recovered() -> i64 { return 0; }\n",
    ));
    assert_eq!(
        output
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == INVALID_RANGE_SYNTAX)
            .count(),
        3,
        "{:?}",
        output.diagnostics,
    );
    assert!(output.ast.declarations.iter().any(|declaration| {
        matches!(declaration, TopLevelDeclaration::Function(function) if function.name.text == "recovered")
    }));
}
