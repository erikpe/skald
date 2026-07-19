//! Source-file ownership, source IDs, byte ranges, spans, and line lookup.
//!
//! Offsets are UTF-8 byte offsets. Human-facing lines and columns are one-based;
//! columns count Unicode scalar values rather than bytes. Tabs count as one
//! column, leaving presentation policy to the diagnostic renderer.

use std::{ops::Range, path::Path};

/// Stable identity assigned to a source in insertion order.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceId(usize);

impl SourceId {
    pub const fn index(self) -> usize {
        self.0
    }
}

/// A half-open UTF-8 byte range: `start..end`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TextRange {
    start: usize,
    end: usize,
}

impl TextRange {
    pub const fn new(start: usize, end: usize) -> Option<Self> {
        if start <= end {
            Some(Self { start, end })
        } else {
            None
        }
    }

    pub const fn empty(offset: usize) -> Self {
        Self {
            start: offset,
            end: offset,
        }
    }

    pub const fn start(self) -> usize {
        self.start
    }

    pub const fn end(self) -> usize {
        self.end
    }

    pub const fn len(self) -> usize {
        self.end - self.start
    }

    pub const fn is_empty(self) -> bool {
        self.start == self.end
    }

    pub const fn as_range(self) -> Range<usize> {
        self.start..self.end
    }
}

/// A byte range associated with one source file.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Span {
    source_id: SourceId,
    range: TextRange,
}

impl Span {
    pub const fn new(source_id: SourceId, range: TextRange) -> Self {
        Self { source_id, range }
    }

    pub const fn empty(source_id: SourceId, offset: usize) -> Self {
        Self::new(source_id, TextRange::empty(offset))
    }

    pub const fn source_id(self) -> SourceId {
        self.source_id
    }

    pub const fn range(self) -> TextRange {
        self.range
    }
}

/// A one-based human-facing source location.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LineColumn {
    pub line: usize,
    pub column: usize,
}

/// Immutable source text and its precomputed line map.
#[derive(Debug)]
pub struct SourceFile {
    id: SourceId,
    path: Box<Path>,
    text: String,
    line_starts: Vec<usize>,
}

impl SourceFile {
    fn new(id: SourceId, path: Box<Path>, text: String) -> Self {
        let mut line_starts = vec![0];
        line_starts.extend(
            text.bytes()
                .enumerate()
                .filter_map(|(offset, byte)| (byte == b'\n').then_some(offset + 1)),
        );

        Self {
            id,
            path,
            text,
            line_starts,
        }
    }

    pub const fn id(&self) -> SourceId {
        self.id
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn len(&self) -> usize {
        self.text.len()
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    pub fn span(&self, start: usize, end: usize) -> Option<Span> {
        if end > self.text.len()
            || !self.text.is_char_boundary(start)
            || !self.text.is_char_boundary(end)
        {
            return None;
        }

        TextRange::new(start, end).map(|range| Span::new(self.id, range))
    }

    pub fn slice(&self, range: TextRange) -> Option<&str> {
        self.text.get(range.as_range())
    }

    pub fn location(&self, offset: usize) -> Option<LineColumn> {
        if offset > self.text.len() || !self.text.is_char_boundary(offset) {
            return None;
        }

        let line_index = self.line_starts.partition_point(|start| *start <= offset) - 1;
        let line_start = self.line_starts[line_index];
        let column = self.text[line_start..offset].chars().count() + 1;

        Some(LineColumn {
            line: line_index + 1,
            column,
        })
    }

    pub fn line_text(&self, line: usize) -> Option<&str> {
        let line_index = line.checked_sub(1)?;
        let start = *self.line_starts.get(line_index)?;
        let end = self
            .line_starts
            .get(line_index + 1)
            .copied()
            .unwrap_or(self.text.len());

        Some(self.text[start..end].trim_end_matches(['\r', '\n']))
    }
}

/// Owns every source participating in one compilation.
#[derive(Debug, Default)]
pub struct SourceDatabase {
    files: Vec<SourceFile>,
}

impl SourceDatabase {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, path: impl AsRef<Path>, text: impl Into<String>) -> SourceId {
        let id = SourceId(self.files.len());
        self.files
            .push(SourceFile::new(id, path.as_ref().into(), text.into()));
        id
    }

    pub fn get(&self, id: SourceId) -> Option<&SourceFile> {
        self.files.get(id.index())
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_ids_follow_stable_insertion_order() {
        let mut sources = SourceDatabase::new();

        let first = sources.add("first.ska", "one");
        let second = sources.add("second.ska", "two");

        assert_eq!(first.index(), 0);
        assert_eq!(second.index(), 1);
        assert_eq!(sources.get(first).unwrap().path(), Path::new("first.ska"));
        assert_eq!(sources.get(second).unwrap().text(), "two");
    }

    #[test]
    fn line_locations_are_one_based_and_utf8_aware() {
        let mut sources = SourceDatabase::new();
        let id = sources.add("unicode.ska", "aé\nxyz\n");
        let source = sources.get(id).unwrap();

        assert_eq!(source.location(0), Some(LineColumn { line: 1, column: 1 }));
        assert_eq!(source.location(1), Some(LineColumn { line: 1, column: 2 }));
        assert_eq!(source.location(3), Some(LineColumn { line: 1, column: 3 }));
        assert_eq!(source.location(4), Some(LineColumn { line: 2, column: 1 }));
        assert_eq!(source.location(8), Some(LineColumn { line: 3, column: 1 }));
        assert_eq!(source.location(2), None, "offset is inside UTF-8 encoding");
    }

    #[test]
    fn spans_and_lines_use_checked_byte_ranges() {
        let mut sources = SourceDatabase::new();
        let id = sources.add("lines.ska", "first\r\nsecond");
        let source = sources.get(id).unwrap();

        let span = source.span(7, 13).unwrap();
        assert_eq!(source.slice(span.range()), Some("second"));
        assert_eq!(source.line_text(1), Some("first"));
        assert_eq!(source.line_text(2), Some("second"));
        assert_eq!(source.line_text(3), None);
        assert_eq!(source.span(9, 3), None);
        assert_eq!(source.span(0, source.len() + 1), None);
    }
}
