use super::*;

#[test]
fn object_casts_parse_at_unary_precedence_with_frozen_ambiguity_rules() {
    let (_, output) = parse_text(
        "fn inspect(ref value: Obj, other: i64) -> i64 {\n\
           ((Leaf) value).read();\n\
           (shared Leaf) value;\n\
           (f)(value);\n\
           return (other) - 1;\n\
         }\n",
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let statements = &function(&output.ast, 0).body.statements;
    let Statement::Expression(method) = &statements[0] else {
        panic!("expected method expression");
    };
    let Expression::Call(call) = &method.expression else {
        panic!("expected call");
    };
    let Expression::MemberAccess(member) = call.callee.as_ref() else {
        panic!("expected member selection");
    };
    let Expression::Grouped(grouped) = member.receiver.as_ref() else {
        panic!("expected grouped cast receiver");
    };
    assert!(matches!(
        grouped.expression.as_ref(),
        Expression::ObjectCast(cast) if matches!(cast.as_ref(), ObjectCastExpr {
            target_mode: ObjectCastTargetMode::Plain,
            ..
        })
    ));

    let Statement::Expression(shared) = &statements[1] else {
        panic!("expected shared cast expression");
    };
    assert!(matches!(
        &shared.expression,
        Expression::ObjectCast(cast) if matches!(cast.as_ref(), ObjectCastExpr {
            target_mode: ObjectCastTargetMode::Shared { .. },
            ..
        })
    ));

    let Statement::Expression(ambiguous) = &statements[2] else {
        panic!("expected ambiguous cast expression");
    };
    assert!(matches!(ambiguous.expression, Expression::ObjectCast(_)));
    assert!(matches!(
        return_value(function(&output.ast, 0)),
        Expression::Binary(_)
    ));
}

#[test]
fn type_tests_have_lower_precedence_than_arithmetic_and_group_explicitly() {
    let (_, output) = parse_text(
        "fn inspect(ref value: Sample) -> bool { return value.age + 1 is Sample; }\n\
         fn grouped(ref value: Sample) -> bool { return (value is Sample); }\n",
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let Expression::TypeTest(test) = return_value(function(&output.ast, 0)) else {
        panic!("outer expression must be a type test");
    };
    assert!(matches!(*test.source, Expression::Binary(_)));
    assert_eq!(test.target.text, "Sample");

    let Expression::Grouped(grouped) = return_value(function(&output.ast, 1)) else {
        panic!("explicit grouping must remain in the AST");
    };
    assert!(matches!(*grouped.expression, Expression::TypeTest(_)));
}

#[test]
fn malformed_type_operations_report_focused_errors_and_recover() {
    let (_, output) = parse_text(
        "fn broken(ref value: Obj) -> unit {\n\
           value is Left is Right;\n\
           cast_alias: Left = value { return; }\n\
           return;\n\
         }\n\
         fn main() -> i64 { return 0; }\n",
    );

    assert!(output.has_errors());
    let codes: Vec<_> = output
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code)
        .collect();
    assert!(codes.contains(&INVALID_TYPE_TEST));
    assert_eq!(function(&output.ast, 1).name.text, "main");
}

#[test]
fn removed_scoped_cast_syntax_is_rejected_and_parser_recovers() {
    let (_, output) = parse_text(concat!(
        "fn broken(ref value: Obj) -> unit {\n",
        "  nar",
        "row ref leaf: Leaf = value { return; }\n",
        "  return;\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(output.has_errors());
    assert_eq!(function(&output.ast, 1).name.text, "main");
}

#[test]
fn cast_rooted_whole_object_assignment_reaches_semantic_rejection() {
    let (_, output) = parse_text(
        "fn invalid(mut ref value: Obj, ref leaf: Leaf) -> unit {\n\
           ((Leaf) value) = leaf;\n\
           return;\n\
         }\n",
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let function = function(&output.ast, 0);
    assert!(matches!(
        function.body.statements[0],
        Statement::ObjectAssignment(_)
    ));
    assert!(matches!(function.body.statements[1], Statement::Return(_)));
}
