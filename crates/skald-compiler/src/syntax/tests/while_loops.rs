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
    for source in [
        "fn main() -> i64 { while true) {} return 0; }",
        "fn main() -> i64 { while () {} return 0; }",
        "fn main() -> i64 { while (true {} return 0; }",
        "fn main() -> i64 { while (true) return 1; return 0; }",
    ] {
        let (_, output) = parse_text(source);
        assert!(output.has_errors(), "source should be rejected: {source}");
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
fn reserved_loop_exits_receive_focused_temporary_diagnostics() {
    let (_, output) =
        parse_text("fn main() -> i64 { while (true) { break; continue; } return 0; }");

    let diagnostics: Vec<_> = output.diagnostics.iter().collect();
    assert_eq!(diagnostics.len(), 2);
    assert!(diagnostics.iter().all(|diagnostic| {
        diagnostic.code == UNSUPPORTED_LOOP_EXIT
            && diagnostic.message.ends_with("is not supported yet")
    }));
    assert_eq!(diagnostics[0].message, "`break` is not supported yet");
    assert_eq!(diagnostics[1].message, "`continue` is not supported yet");
}
