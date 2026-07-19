use crate::source::SourceDatabase;

use super::*;

#[test]
fn diagnostics_retain_structure_and_error_state() {
    let mut sources = SourceDatabase::new();
    let source_id = sources.add("test.ska", "let");
    let source = sources.get(source_id).unwrap();
    let span = source.span(0, 3).unwrap();
    let mut diagnostics = Diagnostics::new();

    diagnostics.push(
        Diagnostic::warning("TEST001", "example warning")
            .with_primary_label(span, "primary")
            .with_note("note"),
    );

    assert_eq!(diagnostics.len(), 1);
    assert!(!diagnostics.has_errors());
    assert_eq!(diagnostics.iter().next().unwrap().code, "TEST001");
}

#[test]
fn rendering_is_stable_and_source_aware() {
    let mut sources = SourceDatabase::new();
    let source_id = sources.add("example.ska", "fn main() { @ }\n");
    let source = sources.get(source_id).unwrap();
    let span = source.span(12, 13).unwrap();
    let diagnostic = Diagnostic::error("LEX001", "unexpected character `@`")
        .with_primary_label(span, "not valid in the M1 grammar")
        .with_note("the first slice accepts only its documented token set");

    assert_eq!(
        render_diagnostic(&sources, &diagnostic),
        concat!(
            "error[LEX001]: unexpected character `@`\n",
            " --> example.ska:1:13\n",
            "   |\n",
            "1 | fn main() { @ }\n",
            "   |             ^ not valid in the M1 grammar\n",
            "  = note: the first slice accepts only its documented token set\n",
        )
    );
}

#[test]
fn rendering_uses_character_columns_for_utf8() {
    let mut sources = SourceDatabase::new();
    let source_id = sources.add("unicode.ska", "é@\n");
    let source = sources.get(source_id).unwrap();
    let span = source.span(2, 3).unwrap();
    let diagnostic =
        Diagnostic::error("LEX001", "unexpected").with_primary_label(span, "invalid here");

    assert!(render_diagnostic(&sources, &diagnostic).contains("unicode.ska:1:2"));
}
