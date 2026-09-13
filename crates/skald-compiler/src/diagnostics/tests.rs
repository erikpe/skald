use crate::source::{SourceDatabase, Span};

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
        .with_primary_label(span, "not a recognized Skald token")
        .with_note("the lexer accepts only the documented token set");

    assert_eq!(
        render_diagnostic(&sources, &diagnostic),
        concat!(
            "error[LEX001]: unexpected character `@`\n",
            " --> example.ska:1:13\n",
            "   |\n",
            "1 | fn main() { @ }\n",
            "   |             ^ not a recognized Skald token\n",
            "  = note: the lexer accepts only the documented token set\n",
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

#[test]
fn batch_rendering_uses_one_ordered_buffer_for_all_diagnostics() {
    let mut sources = SourceDatabase::new();
    let source_id = sources.add("mixed.ska", "é\tbad\n");
    let source = sources.get(source_id).unwrap();
    let valid_span = source.span(3, 6).unwrap();
    let invalid_span = Span::empty(source_id, source.len() + 1);
    let diagnostics: Diagnostics = [
        Diagnostic::warning("TEST001", "unicode and tab")
            .with_primary_label(valid_span, "selected")
            .with_note("first"),
        Diagnostic::error("TEST002", "invalid location")
            .with_primary_label(invalid_span, "not rendered"),
    ]
    .into_iter()
    .collect();

    assert_eq!(
        render_diagnostics(&sources, &diagnostics),
        concat!(
            "warning[TEST001]: unicode and tab\n",
            " --> mixed.ska:1:3\n",
            "   |\n",
            "1 | é\tbad\n",
            "   |  \t^^^ selected\n",
            "  = note: first\n",
            "\n",
            "error[TEST002]: invalid location\n",
            " --> mixed.ska:<invalid span>\n",
        )
    );
}

#[test]
fn rendering_empty_diagnostics_produces_an_empty_buffer() {
    assert_eq!(
        render_diagnostics(&SourceDatabase::new(), &Diagnostics::new()),
        ""
    );
}
