use super::*;

#[test]
fn primitive_casts_parse_at_unary_precedence() {
    let (sources, output) = parse_text(
        "fn cast(value: u64) -> i64 {\n\
           var small: u8 = (u8) value;\n\
           var nested: u8 = (u8) (u64) -1;\n\
           return (i64) (value);\n\
         }\n",
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let function = function(&output.ast, 0);
    let Statement::Local(first) = &function.body.statements[0] else {
        panic!("expected local declaration");
    };
    let Expression::PrimitiveCast(first) = &first.initializer else {
        panic!("expected primitive cast");
    };
    assert_eq!(first.target, PrimitiveType::U8);
    assert_eq!(
        sources
            .get(first.target_span.source_id())
            .and_then(|source| source.slice(first.target_span.range())),
        Some("u8")
    );
    assert!(matches!(first.source.as_ref(), Expression::Identifier(_)));

    let Statement::Local(nested) = &function.body.statements[1] else {
        panic!("expected nested local declaration");
    };
    let Expression::PrimitiveCast(outer) = &nested.initializer else {
        panic!("expected outer cast");
    };
    let Expression::PrimitiveCast(inner) = outer.source.as_ref() else {
        panic!("expected nested cast");
    };
    assert_eq!(outer.target, PrimitiveType::U8);
    assert_eq!(inner.target, PrimitiveType::U64);
    assert!(matches!(inner.source.as_ref(), Expression::Unary(_)));

    let Expression::PrimitiveCast(returned) = return_value(function) else {
        panic!("primitive keyword followed by a group must remain a cast");
    };
    assert!(matches!(returned.source.as_ref(), Expression::Grouped(_)));
}

#[test]
fn all_primitive_keywords_select_primitive_cast_syntax() {
    let (_, output) = parse_text(
        "fn cast(value: i64) -> unit {\n\
           (i64) value;\n\
           (u64) value;\n\
           (u8) value;\n\
           (f64) value;\n\
           (bool) value;\n\
         }\n",
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let expected = [
        PrimitiveType::I64,
        PrimitiveType::U64,
        PrimitiveType::U8,
        PrimitiveType::F64,
        PrimitiveType::Bool,
    ];
    for (statement, target) in function(&output.ast, 0)
        .body
        .statements
        .iter()
        .zip(expected)
    {
        let Statement::Expression(statement) = statement else {
            panic!("expected expression statement");
        };
        let Expression::PrimitiveCast(cast) = &statement.expression else {
            panic!("primitive keyword must select primitive-cast syntax");
        };
        assert_eq!(cast.target, target);
    }

    let dump = dump_ast(&output.ast);
    assert_eq!(dump, dump_ast(&output.ast));
    assert!(dump.contains("PrimitiveCast target f64"));
    assert!(dump.contains("PrimitiveCast target bool"));
}

#[test]
fn primitive_and_nominal_cast_syntax_remain_disjoint() {
    let (_, output) = parse_text(
        "fn inspect(ref value: Obj, number: u64) -> unit {\n\
           (Leaf) value;\n\
           (shared Leaf) value;\n\
           (i64) number;\n\
           ((i64) number)(1);\n\
         }\n",
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let statements = &function(&output.ast, 0).body.statements;
    let Statement::Expression(nominal) = &statements[0] else {
        panic!("expected expression statement");
    };
    let Statement::Expression(shared) = &statements[1] else {
        panic!("expected expression statement");
    };
    let Statement::Expression(integer) = &statements[2] else {
        panic!("expected expression statement");
    };
    let Statement::Expression(postfix) = &statements[3] else {
        panic!("expected expression statement");
    };
    assert!(matches!(nominal.expression, Expression::ObjectCast(_)));
    assert!(matches!(shared.expression, Expression::ObjectCast(_)));
    assert!(matches!(integer.expression, Expression::PrimitiveCast(_)));
    let Expression::Call(call) = &postfix.expression else {
        panic!("grouped cast must permit a postfix chain");
    };
    let Expression::Grouped(grouped) = call.callee.as_ref() else {
        panic!("postfix use must retain explicit grouping");
    };
    assert!(matches!(
        grouped.expression.as_ref(),
        Expression::PrimitiveCast(_)
    ));
}

#[test]
fn malformed_primitive_cast_recovers_at_a_later_declaration() {
    let (_, output) = parse_text(
        "fn broken() -> i64 { return (i64); }\n\
         fn main() -> i64 { return 0; }\n",
    );

    assert!(output.has_errors());
    assert_eq!(
        function(&output.ast, output.ast.declarations.len() - 1)
            .name
            .text,
        "main"
    );
}
