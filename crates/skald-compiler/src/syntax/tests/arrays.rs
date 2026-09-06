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
        TypeKind::Optional {
            spelling: OptionalTypeSpelling::SharedShorthand,
            ..
        }
    ));

    let dump = dump_ast(&output.ast);
    assert!(dump.contains("Type Grouped"));
    assert!(dump.contains("Type Optional SharedShorthand"));
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
fn grouped_canonical_optional_owner_array_construction_uses_type_lookahead() {
    let (_, output) = parse_text(concat!(
        "class T { init() {} }\n",
        "fn main() -> i64 {\n",
        "  var values: ((shared T)?)[] = ((shared T)?)[]{none};\n",
        "  return 0;\n",
        "}\n",
    ));
    assert!(!output.has_errors(), "{:?}", output.diagnostics);

    let main = function(&output.ast, 1);
    assert!(matches!(
        local(main, 0).initializer,
        Expression::ArrayConstruction(_)
    ));
}

#[test]
fn retains_ordered_array_element_lists_and_exact_punctuation_spans() {
    let text = concat!(
        "class Item { init() {} }\n",
        "fn main(owner: shared Item) -> i64 {\n",
        "  var empty: i64[] = i64[]{};\n",
        "  var one: i64[] = i64[]{1};\n",
        "  var many: i64[] = i64[]{1,\n",
        "    2, 3};\n",
        "  var nested: i64[][] = i64[][]{i64[]{1}, i64[]{2, 3}};\n",
        "  var grouped: (shared Item)[] = (shared Item)[]{owner};\n",
        "  var shared_values: shared i64[] = new i64[]{1, 2};\n",
        "  return i64[]{7, 8}[0];\n",
        "}\n",
    );
    let (sources, output) = parse_text(text);
    assert!(!output.has_errors(), "{:?}", output.diagnostics);

    let main = function(&output.ast, 1);
    let Expression::ArrayConstruction(many) = &local(main, 2).initializer else {
        panic!("expected element-list construction");
    };
    let ArrayConstructionArguments::Elements(list) = &many.arguments else {
        panic!("expected retained element-list source structure");
    };
    assert_eq!(list.elements.len(), 3);
    assert_eq!(list.comma_spans.len(), 2);
    let source = sources.get(list.left_brace_span.source_id()).unwrap();
    assert_eq!(source.slice(list.left_brace_span.range()), Some("{"));
    assert_eq!(source.slice(list.right_brace_span.range()), Some("}"));
    assert!(list
        .comma_spans
        .iter()
        .all(|span| source.slice(span.range()) == Some(",")));
    assert_eq!(source.slice(many.span.range()), Some("i64[]{1,\n    2, 3}"));

    let Expression::ArrayConstruction(empty) = &local(main, 0).initializer else {
        panic!("expected empty element-list construction");
    };
    assert!(matches!(
        &empty.arguments,
        ArrayConstructionArguments::Elements(list)
            if list.elements.is_empty() && list.comma_spans.is_empty()
    ));
    assert!(matches!(
        &local(main, 3).initializer,
        Expression::ArrayConstruction(construction)
            if matches!(
                &construction.arguments,
                ArrayConstructionArguments::Elements(list)
                    if matches!(list.elements.first(), Some(Expression::ArrayConstruction(_)))
            )
    ));
    assert!(matches!(
        return_value(main),
        Expression::BracketProjection(projection)
            if matches!(&*projection.receiver, Expression::ArrayConstruction(_))
    ));

    let dump = dump_ast(&output.ast);
    assert_eq!(dump.matches("Elements @").count(), 9);
    assert_eq!(dump.matches("Comma @").count(), 6);
    assert_eq!(dump, dump_ast(&output.ast));
}

#[test]
fn retains_indexed_array_initializers_and_exact_punctuation_spans() {
    let text = concat!(
        "fn main() -> i64 {\n",
        "  var squares: i64[] = i64[](3u; index => index * index);\n",
        "  var shared_values: shared i64[] = new i64[](2u; index => index);\n",
        "  var rows: i64[][] = i64[][](2u; row =>\n",
        "    i64[](2u; column => row + column));\n",
        "  return i64[](1u; index => index)[0];\n",
        "}\n",
    );
    let (sources, output) = parse_text(text);
    assert!(!output.has_errors(), "{:?}", output.diagnostics);

    let main = function(&output.ast, 0);
    let Expression::ArrayConstruction(squares) = &local(main, 0).initializer else {
        panic!("expected indexed array construction");
    };
    let ArrayConstructionArguments::Indexed(initializer) = &squares.arguments else {
        panic!("expected retained indexed initializer");
    };
    let source = sources
        .get(initializer.left_paren_span.source_id())
        .unwrap();
    assert_eq!(source.slice(initializer.left_paren_span.range()), Some("("));
    assert_eq!(source.slice(initializer.semicolon_span.range()), Some(";"));
    assert_eq!(
        source.slice(initializer.binding.span.range()),
        Some("index")
    );
    assert_eq!(source.slice(initializer.arrow_span.range()), Some("=>"));
    assert_eq!(
        source.slice(initializer.right_paren_span.range()),
        Some(")")
    );
    assert_eq!(
        source.slice(squares.span.range()),
        Some("i64[](3u; index => index * index)")
    );

    assert!(matches!(
        &local(main, 1).initializer,
        Expression::ArrayConstruction(construction)
            if construction.new_span.is_some()
                && matches!(construction.arguments, ArrayConstructionArguments::Indexed(_))
    ));
    assert!(matches!(
        &local(main, 2).initializer,
        Expression::ArrayConstruction(construction)
            if matches!(
                &construction.arguments,
                ArrayConstructionArguments::Indexed(outer)
                    if matches!(&*outer.element, Expression::ArrayConstruction(_))
            )
    ));
    assert!(matches!(
        return_value(main),
        Expression::BracketProjection(projection)
            if matches!(&*projection.receiver, Expression::ArrayConstruction(_))
    ));

    let dump = dump_ast(&output.ast);
    assert_eq!(dump.matches("Arguments Indexed @").count(), 5);
    assert_eq!(dump.matches("FatArrow @").count(), 5);
    assert_eq!(dump, dump_ast(&output.ast));
}

#[test]
fn malformed_indexed_array_initializers_recover_at_clear_boundaries() {
    for malformed in [
        "var values: i64[] = i64[](; index => index);",
        "var values: i64[] = i64[](3u; => 1);",
        "var values: i64[] = i64[](3u; index index);",
        "var values: i64[] = i64[](3u; index => );",
        "var values: i64[] = i64[](3u; index => index;",
        "var values: i64[] = i64[](3u index => index);",
        "var values: i64[] = i64[](3u; index => index, index);",
    ] {
        let source = format!(
            "fn broken() -> i64 {{ {malformed} return 1; }}\n\
             fn recovered() -> i64 {{ return 0; }}\n"
        );
        let (_, output) = parse_text(&source);

        assert!(output.has_errors(), "{malformed} unexpectedly parsed");
        assert_eq!(
            output.diagnostics.len(),
            1,
            "{malformed} should produce one owning syntax diagnostic: {:?}",
            output.diagnostics
        );
        assert!(
            output.ast.declarations.iter().any(|declaration| matches!(
                declaration,
                TopLevelDeclaration::Function(function) if function.name.text == "recovered"
            )),
            "parser did not recover after {malformed}: {:?}",
            output.diagnostics
        );
    }
}

#[test]
fn malformed_array_element_lists_recover_at_clear_boundaries() {
    for malformed in [
        "var values: i64[] = i64[]{1,};",
        "var values: i64[] = i64[]{, 1};",
        "var values: i64[] = i64[]{1,, 2};",
        "var values: i64[] = i64[]{1 2};",
        "var values: i64[] = i64[]{1, 2;",
        "var values: i64[] = [1, 2];",
        "var values: i64[] = {1, 2};",
    ] {
        let source = format!(
            "fn broken() -> i64 {{ {malformed} return 1; }}\n\
             fn recovered() -> i64 {{ return 0; }}\n"
        );
        let (_, output) = parse_text(&source);

        assert!(output.has_errors(), "{malformed} unexpectedly parsed");
        let broken = output
            .ast
            .declarations
            .iter()
            .find_map(|declaration| match declaration {
                TopLevelDeclaration::Function(function) if function.name.text == "broken" => {
                    Some(function)
                }
                _ => None,
            });
        assert!(
            broken.is_some_and(|function| function
                .body
                .statements
                .iter()
                .any(|statement| matches!(statement, Statement::Return(_)))),
            "parser did not preserve the later statement after {malformed}: {:?}",
            output.diagnostics
        );
        assert!(
            output.ast.declarations.iter().any(|declaration| matches!(
                declaration,
                TopLevelDeclaration::Function(function) if function.name.text == "recovered"
            )),
            "parser did not recover after {malformed}: {:?}",
            output.diagnostics
        );
    }
}

#[test]
fn inline_optional_array_payloads_cross_the_syntax_boundary() {
    let (_, output) =
        parse_text("fn broken(value: i64[]?) -> i64 { return 0; } fn main() -> i64 { return 0; }");

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert_eq!(output.ast.declarations.len(), 2);
    assert!(matches!(
        function(&output.ast, 0).parameters[0].type_syntax.kind,
        TypeKind::Optional { .. }
    ));
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
        "var values: i64[] = i64[](1u); try { return values[0]; }",
        "static values: i64[] = i64[]();",
        "var values: atomic i64[] = i64[]();",
    ] {
        let source = format!("fn main() -> i64 {{ {body} return 0; }}");
        let (_, output) = parse_text(&source);
        assert!(output.has_errors(), "{body} unexpectedly entered the AST");
    }
}
