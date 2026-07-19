//! Structured diagnostics shared by pipeline phases.
//!
//! User errors are data, not Rust panics. Rendering is deterministic and kept
//! separate from diagnostic construction so tests and future IDE consumers can
//! inspect structure directly.

use std::fmt::Write;

use crate::source::{LineColumn, SourceDatabase, Span};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Severity {
    Error,
    Warning,
}

impl Severity {
    const fn name(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warning => "warning",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LabelStyle {
    Primary,
    Secondary,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Label {
    pub style: LabelStyle,
    pub span: Span,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: &'static str,
    pub message: String,
    pub labels: Vec<Label>,
    pub notes: Vec<String>,
}

impl Diagnostic {
    pub fn error(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            code,
            message: message.into(),
            labels: Vec::new(),
            notes: Vec::new(),
        }
    }

    pub fn warning(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            code,
            message: message.into(),
            labels: Vec::new(),
            notes: Vec::new(),
        }
    }

    pub fn with_primary_label(mut self, span: Span, message: impl Into<String>) -> Self {
        self.labels.push(Label {
            style: LabelStyle::Primary,
            span,
            message: message.into(),
        });
        self
    }

    pub fn with_secondary_label(mut self, span: Span, message: impl Into<String>) -> Self {
        self.labels.push(Label {
            style: LabelStyle::Secondary,
            span,
            message: message.into(),
        });
        self
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }
}

#[derive(Debug, Default)]
pub struct Diagnostics {
    items: Vec<Diagnostic>,
}

impl Diagnostics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, diagnostic: Diagnostic) {
        self.items.push(diagnostic);
    }

    pub fn iter(&self) -> impl Iterator<Item = &Diagnostic> {
        self.items.iter()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn has_errors(&self) -> bool {
        self.items
            .iter()
            .any(|diagnostic| diagnostic.severity == Severity::Error)
    }

    pub fn into_vec(self) -> Vec<Diagnostic> {
        self.items
    }

    pub fn append(&mut self, other: Self) {
        self.items.extend(other.items);
    }
}

impl FromIterator<Diagnostic> for Diagnostics {
    fn from_iter<T: IntoIterator<Item = Diagnostic>>(iter: T) -> Self {
        Self {
            items: iter.into_iter().collect(),
        }
    }
}

/// Renders one diagnostic without terminal colors or environment-dependent paths.
pub fn render_diagnostic(sources: &SourceDatabase, diagnostic: &Diagnostic) -> String {
    let mut rendered = String::new();
    let _ = writeln!(
        rendered,
        "{}[{}]: {}",
        diagnostic.severity.name(),
        diagnostic.code,
        diagnostic.message
    );

    if let Some(primary) = diagnostic
        .labels
        .iter()
        .find(|label| label.style == LabelStyle::Primary)
        .or_else(|| diagnostic.labels.first())
    {
        render_location_header(&mut rendered, sources, primary.span);
    }

    for label in &diagnostic.labels {
        render_label(&mut rendered, sources, label);
    }

    for note in &diagnostic.notes {
        let _ = writeln!(rendered, "  = note: {note}");
    }

    rendered
}

pub fn render_diagnostics(sources: &SourceDatabase, diagnostics: &Diagnostics) -> String {
    diagnostics
        .iter()
        .map(|diagnostic| render_diagnostic(sources, diagnostic))
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_location_header(rendered: &mut String, sources: &SourceDatabase, span: Span) {
    let Some(source) = sources.get(span.source_id()) else {
        let _ = writeln!(rendered, " --> <unknown source>");
        return;
    };
    let Some(location) = source.location(span.range().start()) else {
        let _ = writeln!(rendered, " --> {}:<invalid span>", source.path().display());
        return;
    };

    let _ = writeln!(
        rendered,
        " --> {}:{}:{}",
        source.path().display(),
        location.line,
        location.column
    );
}

fn render_label(rendered: &mut String, sources: &SourceDatabase, label: &Label) {
    let Some(source) = sources.get(label.span.source_id()) else {
        return;
    };
    let Some(start) = source.location(label.span.range().start()) else {
        return;
    };
    let Some(line_text) = source.line_text(start.line) else {
        return;
    };

    let marker = match label.style {
        LabelStyle::Primary => '^',
        LabelStyle::Secondary => '-',
    };
    let marker_count = label_width(source, label.span, start);
    let gutter_width = start.line.to_string().len();
    let indentation: String = line_text
        .chars()
        .take(start.column.saturating_sub(1))
        .map(|character| if character == '\t' { '\t' } else { ' ' })
        .collect();

    let _ = writeln!(rendered, " {:gutter_width$} |", "");
    let _ = writeln!(rendered, "{} | {line_text}", start.line);
    let _ = write!(
        rendered,
        " {:gutter_width$} | {indentation}{}",
        "",
        marker.to_string().repeat(marker_count)
    );
    if !label.message.is_empty() {
        let _ = write!(rendered, " {}", label.message);
    }
    rendered.push('\n');
}

fn label_width(source: &crate::source::SourceFile, span: Span, start: LineColumn) -> usize {
    let Some(end) = source.location(span.range().end()) else {
        return 1;
    };

    if start.line == end.line {
        end.column.saturating_sub(start.column).max(1)
    } else {
        source
            .line_text(start.line)
            .map(|line| line.chars().count() + 1 - start.column)
            .unwrap_or(1)
            .max(1)
    }
}

#[cfg(test)]
mod tests {
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
}
