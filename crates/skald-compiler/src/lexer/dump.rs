use crate::{dump_format::write_quoted, source::SourceFile};

use super::Token;

pub fn dump_tokens(source: &SourceFile, tokens: &[Token]) -> String {
    let mut dump = String::new();

    for token in tokens {
        assert_eq!(
            token.span.source_id(),
            source.id(),
            "token span must belong to the source being dumped"
        );
        let start = source
            .location(token.span.range().start())
            .expect("token start must be a valid source boundary");
        let end = source
            .location(token.span.range().end())
            .expect("token end must be a valid source boundary");
        let lexeme = source
            .slice(token.span.range())
            .expect("token span must belong to its source");

        dump.push_str(token.kind.name());
        dump.push(' ');
        dump.push_str(&start.line.to_string());
        dump.push(':');
        dump.push_str(&start.column.to_string());
        dump.push_str("..");
        dump.push_str(&end.line.to_string());
        dump.push(':');
        dump.push_str(&end.column.to_string());
        dump.push(' ');
        write_quoted(&mut dump, lexeme);
        dump.push('\n');
    }

    dump
}
