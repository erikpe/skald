//! Shared primitives for deterministic compiler IR dumps.

use std::fmt::Write;

use crate::source::Span;

pub(crate) fn write_indentation(output: &mut String, levels: usize) {
    for _ in 0..levels {
        output.push_str("  ");
    }
}

pub(crate) fn write_quoted(output: &mut String, text: &str) {
    output.push('"');
    for character in text.chars() {
        output.extend(character.escape_default());
    }
    output.push('"');
}

pub(crate) fn write_span(output: &mut String, span: Span) {
    let _ = write!(output, " @{}..{}", span.range().start(), span.range().end());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::SourceDatabase;

    #[test]
    fn writes_the_shared_dump_syntax_exactly() {
        let mut sources = SourceDatabase::new();
        let source = sources.add("test.ska", "abcdef");
        let span = sources.get(source).unwrap().span(1, 4).unwrap();
        let mut output = String::new();

        write_indentation(&mut output, 2);
        write_quoted(&mut output, "a\n\"");
        write_span(&mut output, span);

        assert_eq!(output, "    \"a\\n\\\"\" @1..4");
    }
}
