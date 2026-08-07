use super::*;

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
    for (source, code, message) in [
        (
            "fn main() -> i64 { if true) { return 1; } return 0; }",
            EXPECTED_TOKEN,
            "expected `(` after `if`",
        ),
        (
            "fn main() -> i64 { if () { return 1; } return 0; }",
            EXPECTED_EXPRESSION,
            "expected a condition after `if (`",
        ),
        (
            "fn main() -> i64 { if (true { return 1; } return 0; }",
            EXPECTED_TOKEN,
            "expected `)` after the `if` condition",
        ),
        (
            "fn main() -> i64 { if (true) elif (false) {} return 0; }",
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
    for (source, code, message) in [
        (
            "fn main() -> i64 { elif (true) {} return 0; }",
            EXPECTED_STATEMENT,
            "`elif` has no matching `if`",
        ),
        (
            "fn main() -> i64 { else {} return 0; }",
            EXPECTED_STATEMENT,
            "`else` has no matching `if`",
        ),
        (
            "fn main() -> i64 { if (false) {} else if (true) {} return 0; }",
            EXPECTED_TOKEN,
            "expected `{` to start a block",
        ),
    ] {
        let (_, output) = parse_text(source);
        assert!(output.has_errors());
        assert!(output
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == code && diagnostic.message == message));
    }
}
