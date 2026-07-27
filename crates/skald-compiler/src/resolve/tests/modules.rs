use super::*;

#[test]
fn single_file_resolution_reports_imports_as_unsupported_module_syntax() {
    let output = resolve_text(
        "import std::Str;\n\
         fn main() -> unit {}\n",
    );

    assert!(output.has_errors());
    let diagnostics = output.diagnostics.iter().collect::<Vec<_>>();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code, UNSUPPORTED_MODULE_SYNTAX);
    assert!(diagnostics[0]
        .message
        .contains("whole-program module compilation"));
}

#[test]
fn qualified_uses_do_not_panic_or_degrade_to_unknown_name_diagnostics() {
    let output = resolve_text(
        "fn main() -> unit {\n\
           std::Str::make();\n\
         }\n",
    );

    assert!(output.has_errors());
    assert!(output
        .diagnostics
        .iter()
        .all(|diagnostic| diagnostic.code == UNSUPPORTED_MODULE_SYNTAX));
}
