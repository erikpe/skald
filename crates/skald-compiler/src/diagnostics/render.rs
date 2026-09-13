//! Deterministic plain-text diagnostic rendering.

use std::fmt::Write;

use crate::source::{LineColumn, SourceDatabase, Span};

use super::{Diagnostic, Diagnostics, Label, LabelStyle};

/// Renders one diagnostic without terminal colors or environment-dependent paths.
pub fn render_diagnostic(sources: &SourceDatabase, diagnostic: &Diagnostic) -> String {
    let mut rendered = String::new();
    render_diagnostic_into(&mut rendered, sources, diagnostic);
    rendered
}

/// Renders diagnostics in insertion order, separated by one blank line.
pub fn render_diagnostics(sources: &SourceDatabase, diagnostics: &Diagnostics) -> String {
    render_diagnostics_from_iter(sources, diagnostics.iter())
}

pub(crate) fn render_diagnostics_from_iter<'a>(
    sources: &SourceDatabase,
    diagnostics: impl IntoIterator<Item = &'a Diagnostic>,
) -> String {
    let mut rendered = String::new();
    for diagnostic in diagnostics {
        if !rendered.is_empty() {
            rendered.push('\n');
        }
        render_diagnostic_into(&mut rendered, sources, diagnostic);
    }
    rendered
}

fn render_diagnostic_into(
    rendered: &mut String,
    sources: &SourceDatabase,
    diagnostic: &Diagnostic,
) {
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
        render_location_header(rendered, sources, primary.span);
    }

    for label in &diagnostic.labels {
        render_label(rendered, sources, label);
    }

    for note in &diagnostic.notes {
        let _ = writeln!(rendered, "  = note: {note}");
    }
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
    let _ = writeln!(rendered, " {:gutter_width$} |", "");
    let _ = writeln!(rendered, "{} | {line_text}", start.line);
    let _ = write!(rendered, " {:gutter_width$} | ", "");
    rendered.extend(
        line_text
            .chars()
            .take(start.column.saturating_sub(1))
            .map(|character| if character == '\t' { '\t' } else { ' ' }),
    );
    rendered.extend(std::iter::repeat_n(marker, marker_count));
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
