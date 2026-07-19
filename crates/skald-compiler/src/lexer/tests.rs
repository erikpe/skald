use super::*;
use crate::{
    diagnostics::render_diagnostics,
    source::{LineColumn, SourceDatabase},
};

fn lex_text(text: &str) -> (SourceDatabase, crate::source::SourceId, LexOutput) {
    let mut sources = SourceDatabase::new();
    let source_id = sources.add("test.ska", text);
    let output = lex(sources.get(source_id).unwrap());
    (sources, source_id, output)
}

#[test]
fn lexes_the_complete_m1_token_surface() {
    let source = "fn add(left: i64, right: i64) -> i64 {\n    var result: i64 = left + right * 2 - 1;\n    return result;\n}";
    let (_, _, output) = lex_text(source);
    let kinds: Vec<_> = output.tokens.iter().map(|token| token.kind).collect();

    assert_eq!(
        kinds,
        vec![
            TokenKind::Fn,
            TokenKind::Identifier,
            TokenKind::LeftParen,
            TokenKind::Identifier,
            TokenKind::Colon,
            TokenKind::I64,
            TokenKind::Comma,
            TokenKind::Identifier,
            TokenKind::Colon,
            TokenKind::I64,
            TokenKind::RightParen,
            TokenKind::Arrow,
            TokenKind::I64,
            TokenKind::LeftBrace,
            TokenKind::Var,
            TokenKind::Identifier,
            TokenKind::Colon,
            TokenKind::I64,
            TokenKind::Equal,
            TokenKind::Identifier,
            TokenKind::Plus,
            TokenKind::Identifier,
            TokenKind::Star,
            TokenKind::IntegerLiteral,
            TokenKind::Minus,
            TokenKind::IntegerLiteral,
            TokenKind::Semicolon,
            TokenKind::Return,
            TokenKind::Identifier,
            TokenKind::Semicolon,
            TokenKind::RightBrace,
            TokenKind::Eof,
        ]
    );
    assert!(!output.has_errors());
}

#[test]
fn recognizes_unit_as_a_keyword() {
    let (_, _, output) = lex_text("unit unit_value");

    assert_eq!(output.tokens[0].kind, TokenKind::Unit);
    assert_eq!(output.tokens[1].kind, TokenKind::Identifier);
    assert!(!output.has_errors());
}

#[test]
fn recognizes_extern_as_a_keyword() {
    let (_, _, output) = lex_text("extern external");

    assert_eq!(output.tokens[0].kind, TokenKind::Extern);
    assert_eq!(output.tokens[1].kind, TokenKind::Identifier);
    assert!(!output.has_errors());
}

#[test]
fn skips_ascii_whitespace_and_line_comments() {
    let (_, _, output) = lex_text("// before\r\n\tvar value: i64 = 7; // after");
    let kinds: Vec<_> = output.tokens.iter().map(|token| token.kind).collect();

    assert_eq!(
        kinds,
        vec![
            TokenKind::Var,
            TokenKind::Identifier,
            TokenKind::Colon,
            TokenKind::I64,
            TokenKind::Equal,
            TokenKind::IntegerLiteral,
            TokenKind::Semicolon,
            TokenKind::Eof,
        ]
    );
    assert!(output.diagnostics.is_empty());
}

#[test]
fn identifiers_are_ascii_and_allow_underscores_and_later_digits() {
    let (sources, source_id, output) = lex_text("_value2 fnx i64_value");
    let source = sources.get(source_id).unwrap();
    let lexemes: Vec<_> = output
        .tokens
        .iter()
        .take(3)
        .map(|token| source.slice(token.span.range()).unwrap())
        .collect();

    assert_eq!(lexemes, vec!["_value2", "fnx", "i64_value"]);
    assert!(output.tokens[..3]
        .iter()
        .all(|token| token.kind == TokenKind::Identifier));
}

#[test]
fn malformed_decimal_spellings_are_single_invalid_tokens() {
    let (sources, source_id, output) = lex_text("12abc 1_000 12.5 0xff");
    let source = sources.get(source_id).unwrap();
    let invalid_lexemes: Vec<_> = output
        .tokens
        .iter()
        .filter(|token| token.kind == TokenKind::Invalid)
        .map(|token| source.slice(token.span.range()).unwrap())
        .collect();

    assert_eq!(invalid_lexemes, vec!["12abc", "1_000", "12.5", "0xff"]);
    assert_eq!(output.diagnostics.len(), 4);
    assert!(output
        .diagnostics
        .iter()
        .all(|diagnostic| diagnostic.code == MALFORMED_INTEGER_LITERAL));
}

#[test]
fn integer_range_is_deliberately_not_checked_by_the_lexer() {
    let (_, _, output) = lex_text("9223372036854775808");

    assert_eq!(output.tokens[0].kind, TokenKind::IntegerLiteral);
    assert!(output.diagnostics.is_empty());
}

#[test]
fn invalid_characters_are_reported_and_lexing_recovers() {
    let (sources, _, output) = lex_text("var x: i64 = @; return x;");

    assert_eq!(output.diagnostics.len(), 1);
    assert_eq!(
        output.diagnostics.iter().next().unwrap().code,
        UNEXPECTED_CHARACTER
    );
    assert!(output
        .tokens
        .iter()
        .any(|token| token.kind == TokenKind::Return));
    assert!(render_diagnostics(&sources, &output.diagnostics).contains("test.ska:1:14"));
}

#[test]
fn utf8_invalid_characters_have_byte_spans_and_character_columns() {
    let (sources, source_id, output) = lex_text("\né;");
    let source = sources.get(source_id).unwrap();
    let invalid = output
        .tokens
        .iter()
        .find(|token| token.kind == TokenKind::Invalid)
        .unwrap();

    assert_eq!(invalid.span.range().start(), 1);
    assert_eq!(invalid.span.range().end(), 3);
    assert_eq!(
        source.location(invalid.span.range().start()),
        Some(LineColumn { line: 2, column: 1 })
    );
    assert_eq!(
        source.location(invalid.span.range().end()),
        Some(LineColumn { line: 2, column: 2 })
    );
}

#[test]
fn token_spans_and_eof_location_are_accurate() {
    let (sources, source_id, output) = lex_text("\n  fn main");
    let source = sources.get(source_id).unwrap();

    let first = output.tokens.first().unwrap();
    let eof = output.tokens.last().unwrap();
    assert_eq!(first.kind, TokenKind::Fn);
    assert_eq!(
        source.location(first.span.range().start()),
        Some(LineColumn { line: 2, column: 3 })
    );
    assert_eq!(eof.kind, TokenKind::Eof);
    assert!(eof.span.range().is_empty());
    assert_eq!(
        source.location(eof.span.range().start()),
        Some(LineColumn {
            line: 2,
            column: 10
        })
    );
}

#[test]
fn token_dump_is_deterministic_and_escapes_lexemes() {
    let (sources, source_id, output) = lex_text("fn x() -> i64 {\n return 7;\n}");
    let source = sources.get(source_id).unwrap();

    assert_eq!(
        dump_tokens(source, &output.tokens),
        concat!(
            "FN 1:1..1:3 \"fn\"\n",
            "IDENTIFIER 1:4..1:5 \"x\"\n",
            "LEFT_PAREN 1:5..1:6 \"(\"\n",
            "RIGHT_PAREN 1:6..1:7 \")\"\n",
            "ARROW 1:8..1:10 \"->\"\n",
            "I64 1:11..1:14 \"i64\"\n",
            "LEFT_BRACE 1:15..1:16 \"{\"\n",
            "RETURN 2:2..2:8 \"return\"\n",
            "INTEGER_LITERAL 2:9..2:10 \"7\"\n",
            "SEMICOLON 2:10..2:11 \";\"\n",
            "RIGHT_BRACE 3:1..3:2 \"}\"\n",
            "EOF 3:2..3:2 \"\"\n",
        )
    );
}

#[test]
#[should_panic(expected = "token span must belong to the source being dumped")]
fn token_dump_rejects_tokens_from_another_source() {
    let mut sources = SourceDatabase::new();
    let first_id = sources.add("first.ska", "fn");
    let second_id = sources.add("second.ska", "fn");
    let tokens = lex(sources.get(first_id).unwrap()).tokens;

    dump_tokens(sources.get(second_id).unwrap(), &tokens);
}
