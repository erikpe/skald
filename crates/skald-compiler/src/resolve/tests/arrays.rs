use super::*;

#[test]
fn arrays_stop_at_the_deliberate_resolution_gate() {
    for source in [
        "fn main(values: i64[]) -> i64 { return 0; }",
        "fn main() -> i64 { var values: i64[] = i64[](4u); return 0; }",
        "fn main() -> i64 { return values[-1]; }",
        "fn main() -> i64 { return owner->[1:]; }",
    ] {
        let output = resolve_text(source);
        assert!(output.has_errors());
        assert!(output
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == UNSUPPORTED_ARRAY_SYNTAX));
    }
}
