use super::*;

#[test]
fn parses_every_shape_as_a_neutral_postfix_operation() {
    let (_, output) = parse_text(concat!(
        "fn main() -> i64 {\n",
        "  values[0]; values[-1]; values[4:-3]; values[:7]; values[2:]; values[:];\n",
        "  owner->[1]; owner->[2:-1]; (*owner)[3].field;\n",
        "  values[1:3] = source[:];\n",
        "  return 0;\n",
        "}\n",
    ));
    assert!(!output.has_errors(), "{:?}", output.diagnostics);

    let dump = dump_ast(&output.ast);
    assert_eq!(dump.matches("BracketProjection").count(), 11);
    assert_eq!(dump.matches("SharedArrow").count(), 2);
    assert_eq!(dump.matches("Colon").count(), 7);
    assert!(dump.contains("MemberAccess"));
    assert!(dump.contains("ObjectAssignment"));
}

#[test]
fn punctuation_and_complete_span_are_preserved() {
    let (sources, output) = parse_text("fn main() -> i64 { return values[4:-3]; }");
    let Expression::BracketProjection(projection) = return_value(function(&output.ast, 0)) else {
        panic!("expected a bracket projection");
    };
    let source = sources.get(projection.span.source_id()).unwrap();
    assert_eq!(source.slice(projection.span.range()), Some("values[4:-3]"));

    let BracketProjectionOperator::Ordinary { left_bracket_span } = projection.operator else {
        panic!("expected ordinary indexing");
    };
    let BracketProjectionBounds::Slice { colon_span, .. } = projection.bounds else {
        panic!("expected a slice");
    };
    assert_eq!(source.slice(left_bracket_span.range()), Some("["));
    assert_eq!(source.slice(colon_span.range()), Some(":"));
    assert_eq!(
        source.slice(projection.right_bracket_span.range()),
        Some("]")
    );
}

#[test]
fn shared_projection_preserves_arrow_and_complete_span() {
    let (sources, output) = parse_text("fn main() -> i64 { return owner->[1]; }");
    let Expression::BracketProjection(projection) = return_value(function(&output.ast, 0)) else {
        panic!("expected a bracket projection");
    };
    let BracketProjectionOperator::Shared {
        arrow_span,
        left_bracket_span,
    } = projection.operator
    else {
        panic!("expected a shared bracket projection");
    };
    let source = sources.get(projection.span.source_id()).unwrap();

    assert_eq!(source.slice(projection.span.range()), Some("owner->[1]"));
    assert_eq!(source.slice(arrow_span.range()), Some("->"));
    assert_eq!(source.slice(left_bracket_span.range()), Some("["));
    assert_eq!(
        source.slice(projection.right_bracket_span.range()),
        Some("]")
    );
}

#[test]
fn malformed_projections_recover_at_later_statements_and_declarations() {
    let (_, output) = parse_text(concat!(
        "fn broken() -> i64 { values[1:; return 1; }\n",
        "fn recovered() -> i64 { values[::]; values[1 2]; return 0; }\n",
    ));

    assert!(output.has_errors());
    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("`]`")));
    assert!(output.ast.declarations.iter().any(|declaration| matches!(
        declaration,
        TopLevelDeclaration::Function(function) if function.name.text == "recovered"
    )));
}

#[test]
fn empty_brackets_report_a_type_neutral_diagnostic() {
    let (_, output) = parse_text(concat!(
        "fn broken() -> i64 { values[]; return 1; }\n",
        "fn recovered() -> i64 { return 0; }\n",
    ));

    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message == "expected an index or slice expression"));
    assert!(output.ast.declarations.iter().any(|declaration| matches!(
        declaration,
        TopLevelDeclaration::Function(function) if function.name.text == "recovered"
    )));
}
