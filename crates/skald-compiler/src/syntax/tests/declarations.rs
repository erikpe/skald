use super::*;

#[test]
fn parses_the_vertical_slice_demonstration_program() {
    let source = concat!(
        "fn twice(value: i64) -> i64 {\n",
        "    return value * 2;\n",
        "}\n",
        "\n",
        "fn main() -> i64 {\n",
        "    var result: i64 = twice(20);\n",
        "    return result + 2;\n",
        "}\n",
    );
    let (_, output) = parse_text(source);

    assert!(!output.has_errors());
    assert_eq!(output.ast.declarations.len(), 2);
    assert_eq!(function(&output.ast, 0).name.text, "twice");
    assert_eq!(function(&output.ast, 0).parameters.len(), 1);
    assert_eq!(function(&output.ast, 1).name.text, "main");
    assert_eq!(function(&output.ast, 1).body.statements.len(), 2);

    let Statement::Local(local) = &function(&output.ast, 1).body.statements[0] else {
        panic!("expected local declaration");
    };
    let Expression::Call(call) = &local.initializer else {
        panic!("expected call initializer");
    };
    assert_eq!(call.arguments.len(), 1);
}

#[test]
fn parses_unit_returns_and_expression_statements_with_complete_spans() {
    let (_, output) = parse_text(concat!(
        "fn notify(value: i64) -> unit { return; }\n",
        "fn main() -> i64 { (notify(7)); return 0; }\n",
    ));

    assert!(!output.has_errors());
    assert_eq!(function(&output.ast, 0).return_type.kind, TypeKind::Unit);
    let Statement::Return(unit_return) = &function(&output.ast, 0).body.statements[0] else {
        panic!("expected unit return");
    };
    assert!(unit_return.value.is_none());
    let Statement::Expression(statement) = &function(&output.ast, 1).body.statements[0] else {
        panic!("expected expression statement");
    };
    assert!(matches!(statement.expression, Expression::Grouped(_)));
    assert_eq!(statement.span.range().start(), 61);
    assert_eq!(statement.span.range().end(), 73);
    let dump = dump_ast(&output.ast);
    assert_eq!(dump, dump_ast(&output.ast));
    assert!(dump.contains("Type Unit"));
    assert!(dump.contains("ExpressionStatement"));
}

#[test]
fn parses_external_functions_as_bodyless_top_level_declarations() {
    let (_, output) = parse_text(concat!(
        "extern fn emit(value: i64) -> unit;\n",
        "fn main() -> i64 { emit(7); return 0; }\n",
    ));

    assert!(!output.has_errors());
    assert_eq!(output.ast.declarations.len(), 2);
    let TopLevelDeclaration::ExternalFunction(external) = &output.ast.declarations[0] else {
        panic!("expected external function declaration");
    };
    assert_eq!(external.name.text, "emit");
    assert_eq!(external.parameters.len(), 1);
    assert_eq!(external.return_type.kind, TypeKind::Unit);
    let dump = dump_ast(&output.ast);
    assert_eq!(dump, dump_ast(&output.ast));
    assert!(dump.contains("ExternalFunction @0..35"));
    assert!(dump.contains("Name \"emit\" @10..14"));
    assert!(dump.contains("Type Unit @30..34"));
}

#[test]
fn unit_is_not_accepted_for_parameter_or_local_storage() {
    for source in [
        "fn bad(value: unit) -> unit {} fn main() -> i64 { return 0; }",
        "extern fn bad(value: unit) -> unit; fn main() -> i64 { return 0; }",
        "fn main() -> i64 { var value: unit = 0; return 0; }",
    ] {
        let (_, output) = parse_text(source);
        assert!(output.has_errors());
        assert!(output.diagnostics.iter().any(|diagnostic| diagnostic
            .labels
            .iter()
            .any(|label| label.message.contains("must have type `i64`"))));
    }
}
