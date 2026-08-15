use super::*;

fn class(ast: &CompilationUnit, index: usize) -> &ClassDecl {
    let TopLevelDeclaration::Class(class) = &ast.declarations[index] else {
        panic!("expected a class declaration");
    };
    class
}

fn source_text(sources: &SourceDatabase, span: crate::source::Span) -> &str {
    sources
        .get(span.source_id())
        .and_then(|source| source.slice(span.range()))
        .expect("AST span must belong to the test source")
}

#[test]
fn generic_class_headers_preserve_parameters_base_and_where_requirements() {
    let (sources, output) = parse_text(
        "class Pair<T, U> extends Base<T> implements Display, api::Hash \
         where T: Comparable, U: api::Hash { first: T; second: U; }",
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let pair = class(&output.ast, 0);
    let parameters = pair.type_parameters.as_ref().expect("type parameters");
    assert_eq!(parameters.parameters[0].text, "T");
    assert_eq!(parameters.parameters[1].text, "U");
    assert_eq!(source_text(&sources, parameters.left_angle_span), "<");
    assert_eq!(source_text(&sources, parameters.comma_spans[0]), ",");
    assert_eq!(source_text(&sources, parameters.right_angle_span), ">");

    let base = pair.direct_base.as_ref().expect("generic direct base");
    assert_eq!(base.name.text, "Base");
    assert_eq!(source_text(&sources, base.span), "Base<T>");
    assert_eq!(base.arguments.as_ref().unwrap().arguments.len(), 1);

    let clause = pair.where_clause.as_ref().expect("where clause");
    assert_eq!(clause.requirements.len(), 2);
    assert_eq!(clause.requirements[0].parameter.text, "T");
    assert_eq!(clause.requirements[0].interface.text, "Comparable");
    assert_eq!(clause.requirements[1].interface.text, "api::Hash");
    assert_eq!(source_text(&sources, clause.where_span), "where");
    assert_eq!(
        source_text(&sources, clause.requirements[0].colon_span),
        ":"
    );
}

#[test]
fn generic_named_types_compose_structurally_in_declarations() {
    let (sources, output) = parse_text(concat!(
        "class Shapes<T> {\n",
        "  nested: Outer<Inner<Str>>;\n",
        "  optional_array: T??[];\n",
        "  shared_optional: (shared Box<T>)?;\n",
        "  fn map(value: Vec<T?>[]) -> shared Result<T> { return value; }\n",
        "}\n",
    ));

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let shapes = class(&output.ast, 0);
    let ClassMember::Field(nested) = &shapes.members[0] else {
        panic!("expected field");
    };
    assert_eq!(
        source_text(&sources, nested.type_syntax.span),
        "Outer<Inner<Str>>"
    );
    let TypeKind::Named(outer) = &nested.type_syntax.kind else {
        panic!("expected generic outer type");
    };
    let inner = &outer.arguments.as_ref().unwrap().arguments[0];
    assert_eq!(source_text(&sources, inner.span), "Inner<Str>");
    let TypeKind::Named(inner) = &inner.kind else {
        panic!("expected nested generic type");
    };
    assert_eq!(
        source_text(&sources, inner.arguments.as_ref().unwrap().right_angle_span,),
        ">"
    );
    assert_eq!(
        source_text(&sources, outer.arguments.as_ref().unwrap().right_angle_span,),
        ">"
    );

    let dump = dump_ast(&output.ast);
    assert!(dump.contains("Type Named @28..45\n"), "{dump}");
    assert!(dump.contains("Arguments @33..45\n"), "{dump}");
    assert!(dump.contains("Type Optional Postfix"), "{dump}");
    assert!(dump.contains("Type Shared"), "{dump}");
}

#[test]
fn nested_type_closers_do_not_change_shift_or_comparison_expressions() {
    let (_, output) = parse_text(
        "class Nested { value: A<B<C<Str>>>; }\n\
         fn shifted(value: u64) -> bool { return value >> 1u > 0u; }",
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let Expression::Binary(comparison) = return_value(function(&output.ast, 1)) else {
        panic!("expected comparison");
    };
    assert_eq!(comparison.operator, BinaryOperator::GreaterThan);
    let Expression::Binary(shift) = comparison.left.as_ref() else {
        panic!("expected shift on the comparison's left");
    };
    assert_eq!(shift.operator, BinaryOperator::ShiftRight);
}

#[test]
fn generic_type_applications_parse_in_all_expression_type_positions() {
    let (sources, output) = parse_text(concat!(
        "class Use<T> {\n",
        "  fn inspect(ref value: Obj) -> unit {\n",
        "    Vec<T>();\n",
        "    new Vec<T>();\n",
        "    Vec<T>.size();\n",
        "    (Vec<T>) value;\n",
        "    value is Vec<T>;\n",
        "    Vec<T>?[]();\n",
        "    new Vec<T>?();\n",
        "    return;\n",
        "  }\n",
        "}\n",
    ));

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let ClassMember::Method(method) = &class(&output.ast, 0).members[0] else {
        panic!("expected method");
    };
    let statements = &method.body.statements;
    let Statement::Expression(construction) = &statements[0] else {
        panic!("expected construction expression");
    };
    let Expression::Call(call) = &construction.expression else {
        panic!("expected constructor call");
    };
    assert!(matches!(
        call.callee.as_ref(),
        Expression::GenericTypeApplication(_)
    ));

    let Statement::Expression(allocation) = &statements[1] else {
        panic!("expected allocation expression");
    };
    assert!(matches!(allocation.expression, Expression::Allocation(_)));

    let Statement::Expression(static_call) = &statements[2] else {
        panic!("expected static call");
    };
    let Expression::Call(call) = &static_call.expression else {
        panic!("expected static call");
    };
    let Expression::GenericStaticSelection(selection) = call.callee.as_ref() else {
        panic!("expected generic static selection")
    };
    assert_eq!(source_text(&sources, selection.separator_span), ".");
    assert!(matches!(
        statements[3],
        Statement::Expression(ExpressionStatement {
            expression: Expression::ObjectCast(_),
            ..
        })
    ));
    assert!(matches!(
        statements[4],
        Statement::Expression(ExpressionStatement {
            expression: Expression::TypeTest(_),
            ..
        })
    ));
    assert!(matches!(
        statements[5],
        Statement::Expression(ExpressionStatement {
            expression: Expression::ArrayConstruction(_),
            ..
        })
    ));
    assert!(matches!(
        statements[6],
        Statement::Expression(ExpressionStatement {
            expression: Expression::OptionalBoxAllocation(_),
            ..
        })
    ));
}

#[test]
fn module_qualified_nested_generic_static_selection_uses_dot() {
    let (sources, output) =
        parse_text("fn use() -> unit { dep::Outer<Inner<i64>>.prepare(); return; }");

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let Statement::Expression(statement) = &function(&output.ast, 0).body.statements[0] else {
        panic!("expected static call statement");
    };
    let Expression::Call(call) = &statement.expression else {
        panic!("expected static call");
    };
    let Expression::GenericStaticSelection(selection) = call.callee.as_ref() else {
        panic!("expected generic static selection");
    };
    assert_eq!(selection.target.name.text, "dep::Outer");
    assert_eq!(selection.member.text, "prepare");
    assert_eq!(source_text(&sources, selection.separator_span), ".");
    assert_eq!(
        source_text(&sources, selection.span),
        "dep::Outer<Inner<i64>>.prepare"
    );
}

#[test]
fn legacy_generic_static_separator_reports_once_and_recovers() {
    let (_, output) = parse_text(concat!(
        "fn legacy() -> unit { Factory<i64>::prepare(); return; }\n",
        "fn recovered() -> i64 { return 0; }\n",
    ));

    let diagnostics = output.diagnostics.iter().collect::<Vec<_>>();
    assert_eq!(diagnostics.len(), 1, "{:?}", output.diagnostics);
    assert_eq!(diagnostics[0].code, INVALID_GENERIC_SYNTAX);
    assert_eq!(
        diagnostics[0].message,
        "generic static members use `.` after the class application"
    );
    assert_eq!(output.ast.declarations.len(), 2);
    assert_eq!(output.ast.declarations[1].name().text, "recovered");
}

#[test]
fn where_remains_an_identifier_outside_a_generic_class_header() {
    let (_, output) =
        parse_text("class Words { where: i64; fn read(where: i64) -> i64 { return where; } }");

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let words = class(&output.ast, 0);
    assert!(words.type_parameters.is_none());
    assert!(words.where_clause.is_none());
    let ClassMember::Field(field) = &words.members[0] else {
        panic!("expected field");
    };
    assert_eq!(field.name.text, "where");
}

#[test]
fn malformed_generic_syntax_reports_and_recovers_to_later_declarations() {
    let (_, output) = parse_text(concat!(
        "class Empty<> {}\n",
        "class Trailing<T,> {}\n",
        "class MissingComma<T U> {}\n",
        "class BadWhere<T> where T Comparable {}\n",
        "class Misplaced<T> where T: Comparable extends Base {}\n",
        "class Good<T> where T: Comparable { value: T; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(output.has_errors());
    assert!(
        output
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == INVALID_GENERIC_SYNTAX)
            .count()
            >= 4,
        "{:?}",
        output.diagnostics
    );
    assert!(output
        .ast
        .declarations
        .iter()
        .any(|declaration| declaration.name().text == "Good"));
    assert!(output
        .ast
        .declarations
        .iter()
        .any(|declaration| declaration.name().text == "main"));
}

#[test]
fn malformed_generic_arguments_recover_without_hiding_the_next_declaration() {
    let cases = [
        "class Broken { value: Vec<>; }",
        "class Broken { value: Vec<i64,>; }",
        "class Broken { value: Vec<i64 u64>; }",
        "class Broken { value: Vec<i64; }",
        "fn broken() -> unit { Vec<>(); return; }",
    ];

    for broken in cases {
        let source = format!("{broken}\nfn recovered() -> i64 {{ return 0; }}\n");
        let (_, output) = parse_text(&source);
        assert!(output.has_errors(), "source unexpectedly parsed: {broken}");
        assert!(
            output
                .ast
                .declarations
                .iter()
                .any(|declaration| declaration.name().text == "recovered"),
            "failed to recover after {broken}: {:?}",
            output.diagnostics
        );
    }
}

#[test]
fn malformed_generic_dump_and_diagnostic_are_stable() {
    let (_, output) = parse_text("class Broken<T,> {}\nclass Good {}\n");

    let diagnostics = output.diagnostics.iter().collect::<Vec<_>>();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, INVALID_GENERIC_SYNTAX);
    assert_eq!(
        diagnostics[0].message,
        "generic parameter lists do not allow a trailing comma"
    );
    assert_eq!(
        dump_ast(&output.ast),
        concat!(
            "CompilationUnit @0..34\n",
            "  Class @0..19\n",
            "    Name \"Broken\" @6..12\n",
            "    TypeParameters @12..16\n",
            "      LeftAngle @12..13\n",
            "      Parameter \"T\" @13..14\n",
            "      Comma @14..15\n",
            "      RightAngle @15..16\n",
            "    Members\n",
            "  Class @20..33\n",
            "    Name \"Good\" @26..30\n",
            "    Members\n",
        )
    );
}

#[test]
fn generic_header_dump_preserves_punctuation_and_grouping() {
    let (_, output) = parse_text("class Box<T> where T: Comparable { value: (T)?; }");
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);

    assert_eq!(
        dump_ast(&output.ast),
        concat!(
            "CompilationUnit @0..49\n",
            "  Class @0..49\n",
            "    Name \"Box\" @6..9\n",
            "    TypeParameters @9..12\n",
            "      LeftAngle @9..10\n",
            "      Parameter \"T\" @10..11\n",
            "      RightAngle @11..12\n",
            "    WhereClause @13..32\n",
            "      Where @13..18\n",
            "      Requirement @19..32\n",
            "        Parameter \"T\" @19..20\n",
            "        Colon @20..21\n",
            "        Interface \"Comparable\" @22..32\n",
            "    Members\n",
            "      Field @35..47\n",
            "        Name \"value\" @35..40\n",
            "        Type Optional Postfix @42..46\n",
            "          Payload\n",
            "            Type Grouped @42..45\n",
            "              LeftParen @42..43\n",
            "              Type Named \"T\" @43..44\n",
            "              RightParen @44..45\n",
            "          Question @45..46\n",
        )
    );
}
