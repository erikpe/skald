use super::*;
use crate::{lexer::lex, source::SourceDatabase, syntax::dump_ast};

fn parse_text(text: &str) -> (SourceDatabase, ParseOutput) {
    let mut sources = SourceDatabase::new();
    let source_id = sources.add("test.ska", text);
    let source = sources.get(source_id).unwrap();
    let lexed = lex(source);
    assert!(lexed.diagnostics.is_empty(), "test source must lex cleanly");
    let parsed = parse(source, &lexed.tokens);
    (sources, parsed)
}

fn function(ast: &CompilationUnit, index: usize) -> &FunctionDecl {
    let TopLevelDeclaration::Function(function) = &ast.declarations[index] else {
        panic!("expected a local function definition");
    };
    function
}

fn return_value(function: &FunctionDecl) -> &Expression {
    let Statement::Return(statement) = function.body.statements.last().unwrap() else {
        panic!("expected final return statement");
    };
    statement.value.as_ref().expect("expected a return value")
}

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
fn parses_boolean_types_and_literals_in_all_supported_positions() {
    let (_, output) = parse_text(concat!(
        "extern fn emit(value: bool) -> bool;\n",
        "fn identity(value: bool) -> bool { var result: bool = value; return result; }\n",
        "fn main() -> i64 { var value: bool = true; emit(false); return 0; }\n",
    ));

    assert!(!output.has_errors());
    let TopLevelDeclaration::ExternalFunction(external) = &output.ast.declarations[0] else {
        panic!("expected external declaration");
    };
    assert_eq!(external.parameters[0].type_syntax.kind, TypeKind::Bool);
    assert_eq!(external.return_type.kind, TypeKind::Bool);
    let main = function(&output.ast, 2);
    let Statement::Local(local) = &main.body.statements[0] else {
        panic!("expected boolean local");
    };
    assert_eq!(local.type_syntax.kind, TypeKind::Bool);
    assert!(matches!(
        local.initializer,
        Expression::Boolean(BooleanExpr { value: true, .. })
    ));

    let dump = dump_ast(&output.ast);
    assert!(dump.contains("Type Bool"));
    assert!(dump.contains("Boolean true"));
    assert!(dump.contains("Boolean false"));
}

#[test]
fn parses_flat_if_elif_else_arms_with_complete_spans() {
    let source = concat!(
        "fn main() -> i64 {\n",
        "  if (true) { return 1; }\n",
        "  elif (false) { return 2; }\n",
        "  elif (true) { return 3; }\n",
        "  else { return 4; }\n",
        "}\n",
    );
    let (_, output) = parse_text(source);

    assert!(!output.has_errors());
    let Statement::Conditional(conditional) = &function(&output.ast, 0).body.statements[0] else {
        panic!("expected conditional statement");
    };
    assert_eq!(conditional.elif_arms.len(), 2);
    assert!(conditional.else_block.is_some());
    assert_eq!(
        &source[conditional.span.range().start()..conditional.span.range().end()],
        concat!(
            "if (true) { return 1; }\n",
            "  elif (false) { return 2; }\n",
            "  elif (true) { return 3; }\n",
            "  else { return 4; }",
        )
    );
    let dump = dump_ast(&output.ast);
    let if_position = dump.find("IfArm").unwrap();
    let first_elif = dump.find("ElifArm").unwrap();
    let else_position = dump.find("ElseArm").unwrap();
    assert!(if_position < first_elif && first_elif < else_position);
    assert_eq!(dump.matches("ElifArm").count(), 2);
}

#[test]
fn conditional_recovery_reports_missing_structure_and_keeps_later_returns() {
    for source in [
        "fn main() -> i64 { if true) { return 1; } return 0; }",
        "fn main() -> i64 { if () { return 1; } return 0; }",
        "fn main() -> i64 { if (true { return 1; } return 0; }",
        "fn main() -> i64 { if (true) elif (false) {} return 0; }",
    ] {
        let (_, output) = parse_text(source);
        assert!(output.has_errors(), "source should be rejected: {source}");
        let main = function(&output.ast, 0);
        assert!(main
            .body
            .statements
            .iter()
            .any(|statement| matches!(statement, Statement::Return(_))));
    }
}

#[test]
fn rejects_standalone_continuations_and_else_if() {
    for (source, message) in [
        (
            "fn main() -> i64 { elif (true) {} return 0; }",
            "`elif` has no matching `if`",
        ),
        (
            "fn main() -> i64 { else {} return 0; }",
            "`else` has no matching `if`",
        ),
        (
            "fn main() -> i64 { if (false) {} else if (true) {} return 0; }",
            "expected `{` to start a block",
        ),
    ] {
        let (_, output) = parse_text(source);
        assert!(output.has_errors());
        assert!(output
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains(message)));
    }
}

#[test]
fn precedence_and_associativity_are_explicit() {
    let (_, output) = parse_text("fn main() -> i64 { return -a * b + c - d; }");
    assert!(!output.has_errors());

    let Expression::Binary(subtract) = return_value(function(&output.ast, 0)) else {
        panic!("outer expression must be subtraction");
    };
    assert_eq!(subtract.operator, BinaryOperator::Subtract);
    let Expression::Binary(add) = subtract.left.as_ref() else {
        panic!("subtraction left side must be addition");
    };
    assert_eq!(add.operator, BinaryOperator::Add);
    let Expression::Binary(multiply) = add.left.as_ref() else {
        panic!("addition left side must be multiplication");
    };
    assert_eq!(multiply.operator, BinaryOperator::Multiply);
    assert!(matches!(
        multiply.left.as_ref(),
        Expression::Unary(UnaryExpr {
            operator: UnaryOperator::Negate,
            ..
        })
    ));
}

#[test]
fn grouping_overrides_binary_precedence_and_preserves_its_span() {
    let (_, output) = parse_text("fn main() -> i64 { return (1 + 2) * 3; }");
    let Expression::Binary(multiply) = return_value(function(&output.ast, 0)) else {
        panic!("expected multiplication");
    };
    let Expression::Grouped(grouped) = multiply.left.as_ref() else {
        panic!("expected grouped left operand");
    };
    assert_eq!(grouped.span.range().start(), 26);
    assert_eq!(grouped.span.range().end(), 33);
    assert!(matches!(
        grouped.expression.as_ref(),
        Expression::Binary(BinaryExpr {
            operator: BinaryOperator::Add,
            ..
        })
    ));
}

#[test]
fn parser_does_not_perform_semantic_name_lookup() {
    let (_, output) =
        parse_text("fn main() -> i64 { var value: i64 = unknown(missing); return value; }");

    assert!(output.diagnostics.is_empty());
    assert_eq!(output.ast.declarations.len(), 1);
}

#[test]
fn malformed_function_does_not_hide_the_next_declaration() {
    let (_, output) = parse_text(concat!(
        "fn broken(value: Missing) -> i64 { return value; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(output.has_errors());
    assert_eq!(output.ast.declarations.len(), 1);
    assert_eq!(function(&output.ast, 0).name.text, "main");
    assert!(!output.diagnostics.is_empty());
}

#[test]
fn missing_punctuation_is_diagnosed_with_useful_recovery() {
    let (_, output) = parse_text(concat!(
        "fn main() -> i64 {\n",
        "    var first i64 = 1\n",
        "    var second: i64 = 2;\n",
        "    return first + second;\n",
        "}\n",
    ));

    assert!(output.has_errors());
    assert_eq!(output.diagnostics.len(), 2);
    assert_eq!(output.ast.declarations.len(), 1);
    assert_eq!(function(&output.ast, 0).body.statements.len(), 3);
    assert!(output
        .diagnostics
        .iter()
        .all(|diagnostic| diagnostic.code == EXPECTED_TOKEN));
}

#[test]
fn independent_statement_errors_are_both_reported() {
    let (_, output) = parse_text(concat!(
        "fn main() -> i64 {\n",
        "    var : i64 = 1;\n",
        "    return +;\n",
        "    return 0;\n",
        "}\n",
    ));

    assert!(output.has_errors());
    assert!(output.diagnostics.len() >= 2);
    assert!(function(&output.ast, 0)
        .body
        .statements
        .iter()
        .any(|statement| matches!(statement, Statement::Return(_))));
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
fn missing_external_semicolon_recovers_at_the_next_declaration() {
    let (_, output) = parse_text(concat!(
        "extern fn emit(value: i64) -> unit\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(output.has_errors());
    assert_eq!(output.ast.declarations.len(), 2);
    assert!(output.diagnostics.iter().any(|diagnostic| diagnostic
        .message
        .contains("`;` after the external function declaration")));
    assert_eq!(function(&output.ast, 1).name.text, "main");
}

#[test]
fn missing_call_statement_semicolon_recovers_at_return() {
    let (_, output) = parse_text(concat!(
        "fn notify() -> unit {}\n",
        "fn main() -> i64 { notify() return 0; }\n",
    ));

    assert!(output.has_errors());
    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.contains("`;` after the call expression")));
    assert!(function(&output.ast, 1)
        .body
        .statements
        .iter()
        .any(|statement| matches!(statement, Statement::Return(_))));
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

#[test]
fn missing_block_close_recovers_at_the_next_function() {
    let (_, output) = parse_text(concat!(
        "fn first() -> i64 { return 1;\n",
        "fn second() -> i64 { return 2; }\n",
    ));

    assert!(output.has_errors());
    assert_eq!(output.ast.declarations.len(), 2);
    assert_eq!(function(&output.ast, 1).name.text, "second");
}

#[test]
fn ast_dump_is_deterministic() {
    let (_, output) = parse_text("fn main() -> i64 { return add(1, -2); }");

    assert_eq!(
        dump_ast(&output.ast),
        concat!(
            "CompilationUnit @0..39\n",
            "  Function @0..39\n",
            "    Name \"main\" @3..7\n",
            "    Parameters\n",
            "    ReturnType\n",
            "      Type I64 @13..16\n",
            "    Block @17..39\n",
            "      Return @19..37\n",
            "        Call @26..36\n",
            "          Callee\n",
            "            Identifier \"add\" @26..29\n",
            "          Arguments\n",
            "            Integer \"1\" @30..31\n",
            "            Unary Negate @33..35\n",
            "              Integer \"2\" @34..35\n",
        )
    );
}
