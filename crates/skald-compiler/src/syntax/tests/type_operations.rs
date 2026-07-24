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
        Expression::ObjectCast(ObjectCastExpr {
            target_mode: ObjectCastTargetMode::Plain,
            ..
        })
    ));

    let Statement::Expression(shared) = &statements[1] else {
        panic!("expected shared cast expression");
    };
    assert!(matches!(
        shared.expression,
        Expression::ObjectCast(ObjectCastExpr {
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
fn parses_checked_narrowing_as_a_scoped_alias_binding() {
    let (_, output) = parse_text(
        "fn inspect(ref value: Obj) -> unit {\n\
           narrow mut ref sample: Sample = (value) { sample.touch(); }\n\
         }\n",
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let Statement::Narrowing(narrowing) = &function(&output.ast, 0).body.statements[0] else {
        panic!("expected checked narrowing statement");
    };
    assert!(narrowing.binding.mut_span.is_some());
    assert_eq!(narrowing.binding.name.text, "sample");
    assert_eq!(narrowing.binding.target.text, "Sample");
    assert!(matches!(narrowing.source, Expression::Grouped(_)));
    assert_eq!(narrowing.body.statements.len(), 1);
}

#[test]
fn narrow_remains_an_ordinary_callable_name_outside_the_binding_form() {
    let (_, output) = parse_text(
        "fn narrow(value: i64) -> unit {}\n\
         fn caller() -> unit { narrow(1); }\n",
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let Statement::Expression(statement) = &function(&output.ast, 1).body.statements[0] else {
        panic!("expected ordinary call statement");
    };
    assert!(matches!(statement.expression, Expression::Call(_)));
}

#[test]
fn malformed_type_operations_report_focused_errors_and_recover() {
    let (_, output) = parse_text(
        "fn broken(ref value: Obj) -> unit {\n\
           value is Left is Right;\n\
           narrow alias: Left = value { return; }\n\
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
    assert!(codes.contains(&INVALID_NARROWING));
    assert_eq!(function(&output.ast, 1).name.text, "main");
}
