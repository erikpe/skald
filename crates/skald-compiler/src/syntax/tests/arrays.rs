use super::*;

fn local(function: &FunctionDecl, index: usize) -> &LocalDecl {
    let Statement::Local(local) = &function.body.statements[index] else {
        panic!("expected a local declaration");
    };
    local
}

#[test]
fn array_type_grouping_preserves_outer_and_element_ownership() {
    let (_, output) = parse_text(concat!(
        "class T { init() {} }\n",
        "fn values(a: shared T[], b: (shared T)[], c: shared? T[], ",
        "d: shared (shared? T)[], e: (shared T[])[]) -> T[][] { return T[][](); }\n",
    ));
    assert!(!output.has_errors(), "{:?}", output.diagnostics);

    let values = function(&output.ast, 1);
    assert!(matches!(
        values.parameters[0].type_syntax.kind,
        TypeKind::Shared { .. }
    ));
    assert!(matches!(
        values.parameters[1].type_syntax.kind,
        TypeKind::Array { .. }
    ));
    assert!(matches!(
        values.parameters[2].type_syntax.kind,
        TypeKind::OptionalShared { .. }
    ));

    let dump = dump_ast(&output.ast);
    assert!(dump.contains("Type Grouped"));
    assert!(dump.contains("Type OptionalShared"));
    assert!(dump.matches("Type Array").count() >= 9);
}

#[test]
fn array_types_parse_in_alias_and_semantically_rejected_element_positions() {
    let (_, output) = parse_text(concat!(
        "fn inspect(ref values: i64[], mut ref nested: (shared? Item)[]) -> unit[] {\n",
        "  return unit[]();\n",
        "}\n",
    ));
    assert!(!output.has_errors(), "{:?}", output.diagnostics);

    let inspect = function(&output.ast, 0);
    assert!(matches!(
        inspect.parameters[0].type_syntax.kind,
        TypeKind::Array { .. }
    ));
    assert!(matches!(
        inspect.parameters[1].type_syntax.kind,
        TypeKind::Array { .. }
    ));
    assert!(matches!(inspect.return_type.kind, TypeKind::Array { .. }));
}

#[test]
fn parses_every_array_construction_mode_without_call_ambiguity() {
    let (_, output) = parse_text(concat!(
        "fn main() -> i64 {\n",
        "  var empty: i64[] = i64[]();\n",
        "  var sized: i64[] = i64[](23u);\n",
        "  var copied: i64[] = i64[](copy sized);\n",
        "  var shared_empty: shared i64[] = new i64[]();\n",
        "  var shared_sized: shared i64[] = new i64[](23u);\n",
        "  var shared_copy: shared i64[] = new i64[](copy sized);\n",
        "  return 0;\n",
        "}\n",
    ));
    assert!(!output.has_errors(), "{:?}", output.diagnostics);

    let main = function(&output.ast, 0);
    for (index, expected_new) in [false, false, false, true, true, true]
        .into_iter()
        .enumerate()
    {
        let Expression::ArrayConstruction(construction) = &local(main, index).initializer else {
            panic!("expected dedicated array construction");
        };
        assert_eq!(construction.new_span.is_some(), expected_new);
        assert!(matches!(
            construction.array_type.kind,
            TypeKind::Array { .. }
        ));
    }
    assert!(matches!(
        &local(main, 0).initializer,
        Expression::ArrayConstruction(construction)
            if matches!(construction.arguments, ArrayConstructionArguments::Empty { .. })
    ));
    assert!(matches!(
        &local(main, 1).initializer,
        Expression::ArrayConstruction(construction)
            if matches!(construction.arguments, ArrayConstructionArguments::Length { .. })
    ));
    assert!(matches!(
        &local(main, 2).initializer,
        Expression::ArrayConstruction(construction)
            if matches!(construction.arguments, ArrayConstructionArguments::Copy { .. })
    ));
}

#[test]
fn parses_index_slice_and_shared_projection_shapes_as_postfix_operations() {
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
    assert_eq!(dump.matches("ArrayProjection").count(), 11);
    assert_eq!(dump.matches("SharedArrow").count(), 2);
    assert_eq!(dump.matches("Colon").count(), 7);
    assert!(dump.contains("MemberAccess"));
    assert!(dump.contains("ObjectAssignment"));
}

#[test]
fn array_bracket_and_colon_spans_are_preserved() {
    let (sources, output) = parse_text("fn main() -> i64 { return values[4:-3]; }");
    let Expression::ArrayProjection(projection) = return_value(function(&output.ast, 0)) else {
        panic!("expected an array projection");
    };
    let ArrayProjectionOperator::Ordinary { left_bracket_span } = projection.operator else {
        panic!("expected ordinary indexing");
    };
    let ArrayProjectionBounds::Slice { colon_span, .. } = projection.bounds else {
        panic!("expected a slice");
    };
    let source = sources.get(left_bracket_span.source_id()).unwrap();

    assert_eq!(source.slice(left_bracket_span.range()), Some("["));
    assert_eq!(source.slice(colon_span.range()), Some(":"));
    assert_eq!(
        source.slice(projection.right_bracket_span.range()),
        Some("]")
    );
}

#[test]
fn malformed_array_brackets_recover_at_later_statements_and_declarations() {
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
fn inline_optional_array_payloads_remain_syntax_errors() {
    let (_, output) =
        parse_text("fn broken(value: i64[]?) -> i64 { return 0; } fn main() -> i64 { return 0; }");

    assert!(output.has_errors());
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == INVALID_OPTIONAL_TYPE
            && diagnostic.message.contains("inline optional array")
    }));
    assert_eq!(output.ast.declarations.len(), 1);
    assert_eq!(function(&output.ast, 0).name.text, "main");
}

#[test]
fn malformed_array_grouping_recovers_without_changing_later_declarations() {
    for malformed in [
        "fn broken(value: (shared Item[]) -> i64 { return 0; }",
        "fn broken(value: shared ((Item[])[]) -> i64 { return 0; }",
        "fn broken() -> i64 { var value: i64[] = (i64[])(2u); return 0; }",
    ] {
        let source = format!("{malformed}\nfn main() -> i64 {{ return 0; }}\n");
        let (_, output) = parse_text(&source);

        assert!(output.has_errors(), "{malformed} unexpectedly parsed");
        assert!(
            output.ast.declarations.iter().any(|declaration| matches!(
                declaration,
                TopLevelDeclaration::Function(function) if function.name.text == "main"
            )),
            "parser did not recover after {malformed}: {:?}",
            output.diagnostics
        );
    }
}

#[test]
fn deferred_array_syntax_remains_rejected() {
    for body in [
        "var values: i64[] = i64[](1u, 2u);",
        "var values: i64[] = [1, 2];",
        "var values: i64[] = i64[](4u); var part: i64[] = values[0:4:2];",
        "var values: i64[] = i64[](1u); var cast: i64[] = (i64[]) values;",
        "var values: i64[] = i64[](1u); var matches: bool = values is i64[];",
        "var values: i64[] = i64[](1u); for (value in values) { return value; }",
        "var values: i64[] = i64[](1u); try { return values[0]; }",
        "static values: i64[] = i64[]();",
        "var values: atomic i64[] = i64[]();",
    ] {
        let source = format!("fn main() -> i64 {{ {body} return 0; }}");
        let (_, output) = parse_text(&source);
        assert!(output.has_errors(), "{body} unexpectedly entered the AST");
    }
}
