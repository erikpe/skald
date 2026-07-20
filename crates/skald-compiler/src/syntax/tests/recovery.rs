use super::*;

#[test]
fn disabled_numeric_literal_recovery_keeps_the_following_statement() {
    let mut sources = SourceDatabase::new();
    let source_id = sources.add(
        "test.ska",
        "fn main() -> i64 { var value: i64 = 1.; return 0; }",
    );
    let source = sources.get(source_id).unwrap();
    let lexed = lex(source);
    assert!(lexed.has_errors());

    let parsed = parse(source, &lexed.tokens);
    assert!(parsed.diagnostics.is_empty());
    let main = function(&parsed.ast, 0);
    assert_eq!(main.body.statements.len(), 1);
    assert!(matches!(main.body.statements[0], Statement::Return(_)));
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
fn missing_block_close_recovers_at_the_next_function() {
    let (_, output) = parse_text(concat!(
        "fn first() -> i64 { return 1;\n",
        "fn second() -> i64 { return 2; }\n",
    ));

    assert!(output.has_errors());
    assert_eq!(output.ast.declarations.len(), 2);
    assert_eq!(function(&output.ast, 1).name.text, "second");
}
