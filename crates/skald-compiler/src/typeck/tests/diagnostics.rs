use super::*;

#[test]
fn independent_errors_accumulate_across_function_contexts() {
    let output = check_text(concat!(
        "fn first() -> i64 { var value: i64 = true; return false; }\n",
        "fn second() -> bool { return 1; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(output.hir.is_none());
    assert_eq!(output.diagnostics.len(), 3);
    assert!(output
        .diagnostics
        .iter()
        .all(|diagnostic| diagnostic.code == TYPE_MISMATCH));
    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.message.starts_with("local initializer")));
    assert_eq!(
        output
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.message.starts_with("return value"))
            .count(),
        2
    );
}

#[test]
fn diagnoses_invalid_unit_and_value_return_forms() {
    for (source, message) in [
        (
            "fn bad() -> unit { return 1; } fn main() -> i64 { return 0; }",
            "cannot return a value",
        ),
        ("fn main() -> i64 { return; }", "must return a value"),
    ] {
        let output = check_text(source);
        assert!(output.hir.is_none());
        assert!(output.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == INVALID_RETURN && diagnostic.message.contains(message)
        }));
        assert!(!output
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == MISSING_RETURN));
    }
}

#[test]
fn rejects_unit_calls_in_value_contexts() {
    let output = check_text(concat!(
        "fn notify() -> unit {}\n",
        "fn main() -> i64 { return notify(); }\n",
    ));

    assert!(output.hir.is_none());
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == TYPE_MISMATCH
            && diagnostic
                .message
                .contains("type `unit` but `i64` is required")
    }));
}
