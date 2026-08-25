use super::*;
use crate::source::Span;

fn source_slice(source: &str, span: Span) -> &str {
    &source[span.range().start()..span.range().end()]
}

#[test]
fn parses_inferred_annotated_and_nested_for_in_with_complete_stable_spans() {
    let source =
        "fn main(values: u64) -> unit { for (item: u64 in values) { for (inner in item) {} } }";
    let (_, output) = parse_text(source);

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let Statement::ForIn(outer) = &function(&output.ast, 0).body.statements[0] else {
        panic!("expected outer for-in statement");
    };
    assert_eq!(source_slice(source, outer.for_span), "for");
    assert_eq!(source_slice(source, outer.left_paren_span), "(");
    assert_eq!(source_slice(source, outer.binding.span), "item");
    assert_eq!(source_slice(source, outer.in_span), "in");
    assert_eq!(source_slice(source, outer.iterable.span()), "values");
    assert_eq!(source_slice(source, outer.right_paren_span), ")");
    assert_eq!(source_slice(source, outer.span), &source[31..83]);

    let annotation = outer.annotation.as_ref().expect("expected item annotation");
    assert_eq!(source_slice(source, annotation.colon_span), ":");
    assert_eq!(source_slice(source, annotation.type_syntax.span), "u64");
    assert_eq!(source_slice(source, annotation.span), ": u64");

    let [Statement::ForIn(inner)] = outer.body.statements.as_slice() else {
        panic!("expected nested for-in statement");
    };
    assert!(inner.annotation.is_none());
    assert_eq!(source_slice(source, inner.binding.span), "inner");
    assert_eq!(source_slice(source, inner.iterable.span()), "item");
    assert_eq!(source_slice(source, inner.span), &source[59..81]);

    let dump = dump_ast(&output.ast);
    assert_eq!(dump, dump_ast(&output.ast));
    let lines = dump
        .lines()
        .filter(|line| {
            let line = line.trim_start();
            line.starts_with("ForIn ")
                || line.starts_with("ForKeyword ")
                || line.starts_with("Binding \"")
                || line.starts_with("Annotation ")
                || line.starts_with("InDelimiter ")
        })
        .map(str::trim)
        .collect::<Vec<_>>();
    assert_eq!(
        lines,
        [
            "ForIn @31..83",
            "ForKeyword @31..34",
            "Binding \"item\" @36..40",
            "Annotation @40..45",
            "InDelimiter @46..48",
            "ForIn @59..81",
            "ForKeyword @59..62",
            "Binding \"inner\" @64..69",
            "InDelimiter @70..72",
        ]
    );
}

#[test]
fn in_remains_an_identifier_outside_the_for_header_delimiter() {
    let (_, output) = parse_text(concat!(
        "fn in(in: u64) -> u64 { return in; }\n",
        "fn main(in: u64) -> unit { for (in in in) {} }\n",
    ));

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    assert_eq!(function(&output.ast, 0).name.text, "in");
    assert_eq!(function(&output.ast, 0).parameters[0].name.text, "in");
    let Statement::ForIn(statement) = &function(&output.ast, 1).body.statements[0] else {
        panic!("expected for-in statement");
    };
    assert_eq!(statement.binding.text, "in");
    assert!(matches!(
        &statement.iterable,
        Expression::Identifier(identifier) if identifier.name.text == "in"
    ));
}

#[test]
fn malformed_for_in_headers_recover_without_swallowing_later_statements() {
    for (source, code, message) in [
        (
            "fn main() -> i64 { for item in values) {} return 0; }",
            EXPECTED_TOKEN,
            "expected `(` after `for`",
        ),
        (
            "fn main() -> i64 { for (: u64 in values) {} return 0; }",
            EXPECTED_TOKEN,
            "expected an item binding after `for (`",
        ),
        (
            "fn main() -> i64 { for (item: ) {} return 0; }",
            EXPECTED_TOKEN,
            "expected the item type `i64`, `u64`, `u8`, `f64`, or `bool`, a class name, or a shared object type",
        ),
        (
            "fn main() -> i64 { for (item values) {} return 0; }",
            EXPECTED_TOKEN,
            "expected contextual `in` after the item binding",
        ),
        (
            "fn main() -> i64 { for (item in) {} return 0; }",
            EXPECTED_EXPRESSION,
            "expected an iterable expression after `in`",
        ),
        (
            "fn main() -> i64 { for (item in values {} return 0; }",
            EXPECTED_TOKEN,
            "expected `)` after the iterable expression",
        ),
        (
            "fn main() -> i64 { for (item in values) return 1; return 0; }",
            EXPECTED_TOKEN,
            "expected `{` to start a block",
        ),
    ] {
        let (_, output) = parse_text(source);
        assert!(output.has_errors(), "source should be rejected: {source}");
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == code && diagnostic.message == message),
            "missing focused diagnostic for {source}: {:?}",
            output.diagnostics
        );
        assert!(
            function(&output.ast, 0)
                .body
                .statements
                .iter()
                .any(|statement| matches!(statement, Statement::Return(_))),
            "recovery must preserve the later return: {source}"
        );
    }
}

#[test]
fn generic_template_bodies_retain_for_in_syntax() {
    let (_, output) = parse_text(concat!(
        "class Container<T> {\n",
        "  fn scan(values: T) -> unit {\n",
        "    for (item: T in values) { for (nested in item) {} }\n",
        "  }\n",
        "}\n",
    ));

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let TopLevelDeclaration::Class(class) = &output.ast.declarations[0] else {
        panic!("expected generic class");
    };
    let ClassMember::Method(method) = &class.members[0] else {
        panic!("expected generic class method");
    };
    assert!(matches!(method.body.statements[0], Statement::ForIn(_)));
}

#[test]
fn iterable_expressions_obey_the_existing_logical_depth_limit() {
    let expression = (0..MAX_LOGICAL_EXPRESSION_DEPTH + 2)
        .map(|_| "true")
        .collect::<Vec<_>>()
        .join(" && ");
    let source = format!("fn main() -> unit {{ for (item in {expression}) {{}} }}");
    let (_, output) = parse_text(&source);

    assert!(output.has_errors());
    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == EXCESSIVE_NESTING));
}
