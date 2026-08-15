use super::*;

#[test]
fn parses_closed_function_types_with_exact_modes_and_punctuation() {
    let (sources, output) = parse_text(
        "fn accept(callback: fn(i64, ref Item, mut ref Item[]) -> fn() -> bool) -> unit {}",
    );
    assert!(!output.has_errors(), "{:?}", output.diagnostics);

    let TypeKind::Function(function_type) =
        &function(&output.ast, 0).parameters[0].type_syntax.kind
    else {
        panic!("expected a function type");
    };
    assert_eq!(function_type.parameters.len(), 3);
    assert_eq!(function_type.comma_spans.len(), 2);
    assert!(matches!(
        function_type.parameters[0].mode,
        FunctionTypeParameterMode::Value
    ));
    assert!(matches!(
        function_type.parameters[1].mode,
        FunctionTypeParameterMode::ReadOnlyAlias { .. }
    ));
    assert!(matches!(
        function_type.parameters[2].mode,
        FunctionTypeParameterMode::MutableAlias { .. }
    ));
    assert!(matches!(function_type.result.kind, TypeKind::Function(_)));

    let source = sources.get(function_type.span.source_id()).unwrap();
    assert_eq!(
        source.slice(function_type.span.range()),
        Some("fn(i64, ref Item, mut ref Item[]) -> fn() -> bool")
    );
    assert_eq!(source.slice(function_type.fn_span.range()), Some("fn"));
    assert_eq!(source.slice(function_type.arrow_span.range()), Some("->"));

    let dump = dump_ast(&output.ast);
    assert_eq!(dump.matches("Type Function").count(), 2);
    assert!(dump.contains("Mode ReadOnlyAlias"));
    assert!(dump.contains("Mode MutableAlias"));
}

#[test]
fn function_types_parse_in_all_ordinary_type_contexts() {
    let (_, output) = parse_text(concat!(
        "class Slots {\n",
        "  callback: fn(i64) -> bool;\n",
        "  static fallback: fn() -> unit;\n",
        "  init(callback: fn(i64) -> bool) { self.callback = callback; }\n",
        "  fn borrow(ref callback: fn(i64) -> bool) -> fn() -> unit {\n",
        "    var local: fn() -> unit = 0; return local;\n",
        "  }\n",
        "}\n",
        "fn array(value: (fn(i64) -> bool)[]) -> unit {}\n",
    ));
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    assert_eq!(dump_ast(&output.ast).matches("Type Function").count(), 7);
}

#[test]
fn function_result_precedence_and_grouped_container_ownership_are_unambiguous() {
    let (_, output) =
        parse_text("fn shapes(a: fn() -> i64?, b: (fn() -> i64)?, c: (fn() -> i64)[]) -> unit {}");
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let parameters = &function(&output.ast, 0).parameters;

    let TypeKind::Function(first) = &parameters[0].type_syntax.kind else {
        panic!("the optional suffix belongs to the function result");
    };
    assert!(matches!(first.result.kind, TypeKind::Optional { .. }));
    assert!(matches!(
        parameters[1].type_syntax.kind,
        TypeKind::Optional { .. }
    ));
    assert!(matches!(
        parameters[2].type_syntax.kind,
        TypeKind::Array { .. }
    ));
}

#[test]
fn malformed_function_type_modes_and_delimiters_report_structured_errors() {
    for (source, message) in [
        (
            "fn bad(value: fn(mut i64) -> unit) -> unit {}",
            "`ref` after `mut` in a function-type parameter",
        ),
        (
            "fn bad(value: fn(ref mut i64) -> unit) -> unit {}",
            "`mut` must precede `ref`",
        ),
        (
            "fn bad(value: fn(i64,) -> unit) -> unit {}",
            "expected a function-type parameter after `,`",
        ),
        (
            "fn bad(value: fn(i64) unit) -> unit {}",
            "`->` after the function-type parameters",
        ),
        (
            "fn bad(value: fn(i64 -> unit) -> unit {}",
            "`)` after the function-type parameters",
        ),
        (
            "fn bad(value: fn(i64) -> ) -> unit {}",
            "expected a function-type result after `->`",
        ),
    ] {
        let (_, output) = parse_text(source);
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message.contains(message)),
            "missing `{message}` in {:?}",
            output.diagnostics
        );
    }
}

#[test]
fn parenthesized_identifier_cast_shape_remains_expression_syntax() {
    let (_, output) = parse_text("fn invoke(f: i64) -> i64 { return (f)(1); }");
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    assert!(matches!(
        return_value(function(&output.ast, 0)),
        Expression::ObjectCast(_)
    ));
}
