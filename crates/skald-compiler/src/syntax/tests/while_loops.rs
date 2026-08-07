use super::*;

#[test]
fn parses_while_as_a_statement_with_complete_stable_spans() {
    let source = "fn main() -> i64 { while (true) { return 1; } return 0; }";
    let (_, output) = parse_text(source);

    assert!(!output.has_errors());
    let Statement::While(statement) = &function(&output.ast, 0).body.statements[0] else {
        panic!("expected while statement");
    };
    assert_eq!(
        &source[statement.while_span.range().start()..statement.while_span.range().end()],
        "while"
    );
    assert_eq!(
        &source[statement.span.range().start()..statement.span.range().end()],
        "while (true) { return 1; }"
    );
    assert!(matches!(statement.condition, Expression::Boolean(_)));
    assert!(matches!(
        statement.body.statements.as_slice(),
        [Statement::Return(_)]
    ));

    let dump = dump_ast(&output.ast);
    let lines: Vec<_> = dump
        .lines()
        .filter(|line| {
            line.trim_start().starts_with("While ")
                || line.trim_start().starts_with("WhileKeyword ")
                || line.trim_start() == "Condition"
                || line.trim_start().starts_with("Boolean ")
        })
        .map(str::trim)
        .collect();
    assert_eq!(
        lines,
        [
            "While @19..45",
            "WhileKeyword @19..24",
            "Condition",
            "Boolean true @26..30",
        ]
    );
}

#[test]
fn while_recovery_requires_parentheses_and_a_body_then_keeps_later_statements() {
    for (source, code, message) in [
        (
            "fn main() -> i64 { while true) {} return 0; }",
            EXPECTED_TOKEN,
            "expected `(` after `while`",
        ),
        (
            "fn main() -> i64 { while () {} return 0; }",
            EXPECTED_EXPRESSION,
            "expected a condition after `while (`",
        ),
        (
            "fn main() -> i64 { while (true {} return 0; }",
            EXPECTED_TOKEN,
            "expected `)` after the `while` condition",
        ),
        (
            "fn main() -> i64 { while (true) return 1; return 0; }",
            EXPECTED_TOKEN,
            "expected `{` to start a block",
        ),
    ] {
        let (_, output) = parse_text(source);
        assert!(output.has_errors(), "source should be rejected: {source}");
        assert!(output
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == code && diagnostic.message == message));
        assert!(
            function(&output.ast, 0)
                .body
                .statements
                .iter()
                .any(|statement| matches!(statement, Statement::Return(_))),
            "recovery must retain a later return: {source}"
        );
    }
}

#[test]
fn parses_break_with_complete_spans_and_recovers_from_non_semicolon_forms() {
    let source = "fn main() -> i64 { while (true) { break; } return 0; }";
    let (_, output) = parse_text(source);
    assert!(!output.has_errors());
    let Statement::While(statement) = &function(&output.ast, 0).body.statements[0] else {
        panic!("expected while statement");
    };
    let [Statement::Break(statement)] = statement.body.statements.as_slice() else {
        panic!("expected break statement");
    };
    assert_eq!(
        &source[statement.break_span.range().start()..statement.break_span.range().end()],
        "break"
    );
    assert_eq!(
        &source[statement.span.range().start()..statement.span.range().end()],
        "break;"
    );
    let dump = dump_ast(&output.ast);
    assert!(dump.contains("Break @34..40"));
    assert!(dump.contains("BreakKeyword @34..39"));

    for malformed in [
        "fn main() -> i64 { while (true) { break return 0; } return 1; }",
        "fn main() -> i64 { while (true) { break 7; } return 1; }",
    ] {
        let (_, output) = parse_text(malformed);
        assert!(output.has_errors());
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == EXPECTED_TOKEN
                && diagnostic.message == "expected `;` after the `break` statement"
        }));
        assert!(
            function(&output.ast, 0)
                .body
                .statements
                .iter()
                .any(|statement| matches!(statement, Statement::Return(_))),
            "recovery must preserve a later statement: {malformed}"
        );
    }
}

#[test]
fn parses_continue_with_complete_spans_and_recovers_from_non_semicolon_forms() {
    let source = "fn main() -> i64 { while (true) { continue; } return 0; }";
    let (_, output) = parse_text(source);
    assert!(!output.has_errors());
    let Statement::While(statement) = &function(&output.ast, 0).body.statements[0] else {
        panic!("expected while statement");
    };
    let [Statement::Continue(statement)] = statement.body.statements.as_slice() else {
        panic!("expected continue statement");
    };
    assert_eq!(
        &source[statement.continue_span.range().start()..statement.continue_span.range().end()],
        "continue"
    );
    assert_eq!(
        &source[statement.span.range().start()..statement.span.range().end()],
        "continue;"
    );
    let dump = dump_ast(&output.ast);
    assert!(dump.contains("Continue @34..43"));
    assert!(dump.contains("ContinueKeyword @34..42"));

    for malformed in [
        "fn main() -> i64 { while (true) { continue return 0; } return 1; }",
        "fn main() -> i64 { while (true) { continue 7; } return 1; }",
    ] {
        let (_, output) = parse_text(malformed);
        assert!(output.has_errors());
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == EXPECTED_TOKEN
                && diagnostic.message == "expected `;` after the `continue` statement"
        }));
        assert!(
            function(&output.ast, 0)
                .body
                .statements
                .iter()
                .any(|statement| matches!(statement, Statement::Return(_))),
            "recovery must preserve a later statement: {malformed}"
        );
    }
}
