use super::*;

#[test]
fn parses_intrinsic_functions_as_distinct_bodyless_declarations() {
    let (_, output) = parse_text(
        "public intrinsic fn panic(message: Message) -> unit;\n\
         fn main() -> i64 { return 0; }\n",
    );

    assert!(!output.has_errors());
    let TopLevelDeclaration::IntrinsicFunction(panic) = &output.ast.declarations[0] else {
        panic!("expected an intrinsic function declaration");
    };
    assert!(matches!(panic.visibility, Visibility::Public { .. }));
    assert_eq!(panic.name.text, "panic");
    assert_eq!(panic.parameters.len(), 1);
    assert_eq!(panic.return_type.kind, TypeKind::Unit);
    let dump = dump_ast(&output.ast);
    assert!(dump.contains("IntrinsicFunction @0..52"));
    assert!(dump.contains("Intrinsic @7..16"));
    assert!(dump.contains("Name \"panic\" @20..25"));
}

#[test]
fn intrinsic_remains_contextual_outside_the_declaration_introducer() {
    let (_, output) = parse_text(
        "fn intrinsic(intrinsic: i64) -> i64 { return intrinsic; }\n\
         fn main() -> i64 { return intrinsic(0); }\n",
    );

    assert!(!output.has_errors());
    assert_eq!(function(&output.ast, 0).name.text, "intrinsic");
    assert_eq!(
        function(&output.ast, 0).parameters[0].name.text,
        "intrinsic"
    );
}

#[test]
fn rejects_bodies_and_malformed_intrinsic_introducers_during_parsing() {
    for source in [
        "intrinsic fn panic() -> unit {} fn main() -> i64 { return 0; }",
        "intrinsic extern fn panic() -> unit; fn main() -> i64 { return 0; }",
        "extern intrinsic fn panic() -> unit; fn main() -> i64 { return 0; }",
    ] {
        let (_, output) = parse_text(source);
        assert!(output.has_errors());
        assert!(output
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == EXPECTED_TOKEN));
    }
}

#[test]
fn intrinsic_declarations_are_not_class_or_interface_members() {
    for source in [
        "class Bad { intrinsic fn panic() -> unit; } fn main() -> i64 { return 0; }",
        "interface Bad { intrinsic fn panic() -> unit; } fn main() -> i64 { return 0; }",
        "class Bad { init() { intrinsic fn panic() -> unit; } } fn main() -> i64 { return 0; }",
    ] {
        let (_, output) = parse_text(source);
        assert!(output.has_errors());
    }
}
