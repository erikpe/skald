//! Token-surface tests for the contract in `docs/language/GRAMMAR.md`.

use super::*;
use crate::{
    diagnostics::render_diagnostics,
    literal::NumericLiteralKind,
    source::{LineColumn, SourceDatabase},
    test_support::lex_source,
};

const I64_LITERAL: TokenKind = TokenKind::NumericLiteral(NumericLiteralKind::I64);
const U64_LITERAL: TokenKind = TokenKind::NumericLiteral(NumericLiteralKind::U64);
const U8_LITERAL: TokenKind = TokenKind::NumericLiteral(NumericLiteralKind::U8);
const F64_LITERAL: TokenKind = TokenKind::NumericLiteral(NumericLiteralKind::F64);

fn lex_text(text: &str) -> (SourceDatabase, crate::source::SourceId, LexOutput) {
    lex_source(text)
}

#[test]
fn lexes_the_complete_supported_token_surface() {
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
            I64_LITERAL,
            TokenKind::Minus,
            I64_LITERAL,
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
fn division_remainder_and_comments_have_distinct_exact_tokens() {
    let text = "/ % // ignored / %\n/ / %% 1/2// tail\n3%2";
    let (sources, source_id, output) = lex_text(text);
    let source = sources.get(source_id).unwrap();
    assert!(!output.has_errors());
    assert_eq!(
        output
            .tokens
            .iter()
            .map(|token| token.kind)
            .collect::<Vec<_>>(),
        [
            TokenKind::Slash,
            TokenKind::Percent,
            TokenKind::Slash,
            TokenKind::Slash,
            TokenKind::Percent,
            TokenKind::Percent,
            I64_LITERAL,
            TokenKind::Slash,
            I64_LITERAL,
            I64_LITERAL,
            TokenKind::Percent,
            I64_LITERAL,
            TokenKind::Eof,
        ]
    );
    assert_eq!(
        output
            .tokens
            .iter()
            .map(|token| source.slice(token.span.range()).unwrap())
            .collect::<Vec<_>>(),
        ["/", "%", "/", "/", "%", "%", "1", "/", "2", "3", "%", "2", ""]
    );
    let dump = dump_tokens(source, &output.tokens);
    assert_eq!(dump, dump_tokens(source, &output.tokens));
    assert!(dump.contains("SLASH"));
    assert!(dump.contains("PERCENT"));
}

#[test]
fn comparison_and_shift_punctuation_use_longest_match() {
    let (sources, source_id, output) = lex_text("== = != ! < <= << > >= >> -> :: &");
    let source = sources.get(source_id).unwrap();

    assert_eq!(
        output
            .tokens
            .iter()
            .map(|token| token.kind)
            .collect::<Vec<_>>(),
        [
            TokenKind::EqualEqual,
            TokenKind::Equal,
            TokenKind::BangEqual,
            TokenKind::Bang,
            TokenKind::Less,
            TokenKind::LessEqual,
            TokenKind::ShiftLeft,
            TokenKind::Greater,
            TokenKind::GreaterEqual,
            TokenKind::ShiftRight,
            TokenKind::Arrow,
            TokenKind::DoubleColon,
            TokenKind::Ampersand,
            TokenKind::Eof,
        ]
    );
    assert_eq!(
        output
            .tokens
            .iter()
            .map(|token| source.slice(token.span.range()).unwrap())
            .collect::<Vec<_>>(),
        ["==", "=", "!=", "!", "<", "<=", "<<", ">", ">=", ">>", "->", "::", "&", "",]
    );
    assert!(output.diagnostics.is_empty());
    let dump = dump_tokens(source, &output.tokens);
    assert_eq!(dump, dump_tokens(source, &output.tokens));
    assert!(dump.contains("LESS_EQUAL"));
    assert!(dump.contains("SHIFT_LEFT"));
    assert!(dump.contains("GREATER_EQUAL"));
    assert!(dump.contains("SHIFT_RIGHT"));
}

#[test]
fn repeated_angle_punctuation_splits_deterministically() {
    let (sources, source_id, output) = lex_text("<<< >>> <<<= >>>=");
    let source = sources.get(source_id).unwrap();
    assert_eq!(
        output
            .tokens
            .iter()
            .map(|token| token.kind)
            .collect::<Vec<_>>(),
        [
            TokenKind::ShiftLeft,
            TokenKind::Less,
            TokenKind::ShiftRight,
            TokenKind::Greater,
            TokenKind::ShiftLeft,
            TokenKind::LessEqual,
            TokenKind::ShiftRight,
            TokenKind::GreaterEqual,
            TokenKind::Eof,
        ]
    );
    assert_eq!(
        output
            .tokens
            .iter()
            .map(|token| source.slice(token.span.range()).unwrap())
            .collect::<Vec<_>>(),
        ["<<", "<", ">>", ">", "<<", "<=", ">>", ">=", ""]
    );
    assert!(output.diagnostics.is_empty());
}

#[test]
fn logical_punctuation_uses_longest_match_with_precise_utf8_spans() {
    let (sources, source_id, output) = lex_text("left&&right ||\nα&&β // && ||\n&&");
    let source = sources.get(source_id).unwrap();

    let logical_tokens: Vec<_> = output
        .tokens
        .iter()
        .filter(|token| matches!(token.kind, TokenKind::AndAnd | TokenKind::OrOr))
        .collect();
    assert_eq!(
        logical_tokens
            .iter()
            .map(|token| token.kind)
            .collect::<Vec<_>>(),
        [
            TokenKind::AndAnd,
            TokenKind::OrOr,
            TokenKind::AndAnd,
            TokenKind::AndAnd,
        ]
    );
    assert!(logical_tokens
        .iter()
        .all(|token| source.slice(token.span.range()).unwrap().len() == 2));
    assert_eq!(
        logical_tokens[2].span.range().start(),
        "left&&right ||\nα".len()
    );

    let dump = dump_tokens(source, &output.tokens);
    assert_eq!(dump, dump_tokens(source, &output.tokens));
    assert_eq!(dump.matches("AND_AND").count(), 3);
    assert_eq!(dump.matches("OR_OR").count(), 1);
}

#[test]
fn eager_bitwise_and_logical_punctuation_is_split_deterministically() {
    let (sources, source_id, output) = lex_text("& | ^ ~ &&& |||");
    let source = sources.get(source_id).unwrap();

    assert_eq!(
        output
            .tokens
            .iter()
            .map(|token| token.kind)
            .collect::<Vec<_>>(),
        [
            TokenKind::Ampersand,
            TokenKind::Pipe,
            TokenKind::Caret,
            TokenKind::Tilde,
            TokenKind::AndAnd,
            TokenKind::Ampersand,
            TokenKind::OrOr,
            TokenKind::Pipe,
            TokenKind::Eof,
        ]
    );
    assert_eq!(
        output
            .tokens
            .iter()
            .map(|token| source.slice(token.span.range()).unwrap())
            .collect::<Vec<_>>(),
        ["&", "|", "^", "~", "&&", "&", "||", "|", ""]
    );
    assert!(output.diagnostics.is_empty());
}

#[test]
fn recognizes_string_literals_as_single_full_span_tokens() {
    let (sources, source_id, output) = lex_text("\"plain\" \"a\\n\\x42\\0\"");
    let source = sources.get(source_id).unwrap();

    assert_eq!(
        output
            .tokens
            .iter()
            .map(|token| token.kind)
            .collect::<Vec<_>>(),
        [
            TokenKind::StringLiteral,
            TokenKind::StringLiteral,
            TokenKind::Eof,
        ]
    );
    assert_eq!(
        output
            .tokens
            .iter()
            .map(|token| source.slice(token.span.range()).unwrap())
            .collect::<Vec<_>>(),
        ["\"plain\"", "\"a\\n\\x42\\0\"", ""]
    );
    assert!(!output.has_errors());
}

#[test]
fn malformed_string_categories_are_single_invalid_tokens() {
    for (text, message) in [
        ("\"bad\\q\"", "unknown string escape"),
        ("\"bad\\x4\"", "malformed hexadecimal string escape"),
        ("\"café\"", "non-ASCII content"),
        ("\"bad\t\"", "non-printable byte"),
        ("\"unterminated", "unterminated string literal"),
    ] {
        let (_, _, output) = lex_text(text);
        assert_eq!(output.tokens[0].kind, TokenKind::Invalid, "{text:?}");
        assert_eq!(output.diagnostics.len(), 1, "{text:?}");
        let diagnostic = output.diagnostics.iter().next().unwrap();
        assert_eq!(diagnostic.code, MALFORMED_STRING_LITERAL);
        assert!(diagnostic.message.contains(message), "{text:?}");
    }
}

#[test]
fn unescaped_string_newline_recovers_at_the_next_line() {
    let (_, _, output) = lex_text("\"bad\nfn");

    assert_eq!(
        output
            .tokens
            .iter()
            .map(|token| token.kind)
            .collect::<Vec<_>>(),
        [TokenKind::Invalid, TokenKind::Fn, TokenKind::Eof]
    );
    assert!(output
        .diagnostics
        .iter()
        .next()
        .unwrap()
        .message
        .contains("unescaped newline"));
}

#[test]
fn recognizes_unit_as_a_keyword() {
    let (_, _, output) = lex_text("unit unit_value");

    assert_eq!(output.tokens[0].kind, TokenKind::Unit);
    assert_eq!(output.tokens[1].kind, TokenKind::Identifier);
    assert!(!output.has_errors());
}

#[test]
fn reserves_none_but_keeps_presence_and_ownership_words_contextual() {
    let (_, _, output) = lex_text("none some shared none_value some_value shared_value ? !");
    let kinds: Vec<_> = output.tokens.iter().map(|token| token.kind).collect();

    assert_eq!(
        kinds,
        [
            TokenKind::None,
            TokenKind::Identifier,
            TokenKind::Identifier,
            TokenKind::Identifier,
            TokenKind::Identifier,
            TokenKind::Identifier,
            TokenKind::Question,
            TokenKind::Bang,
            TokenKind::Eof,
        ]
    );
    assert!(!output.has_errors());
}

#[test]
fn optional_punctuation_preserves_utf8_boundaries_during_recovery() {
    let (sources, source_id, output) = lex_text("none?é!");
    let source = sources.get(source_id).unwrap();
    let spellings: Vec<_> = output
        .tokens
        .iter()
        .map(|token| source.slice(token.span.range()).unwrap())
        .collect();

    assert_eq!(spellings, ["none", "?", "é", "!", ""]);
    assert_eq!(
        output
            .tokens
            .iter()
            .map(|token| token.kind)
            .collect::<Vec<_>>(),
        [
            TokenKind::None,
            TokenKind::Question,
            TokenKind::Invalid,
            TokenKind::Bang,
            TokenKind::Eof,
        ]
    );
    assert_eq!(output.diagnostics.len(), 1);
}

#[test]
fn brackets_are_independent_punctuation_tokens() {
    let (sources, source_id, output) = lex_text("T[][4:-1]");
    let source = sources.get(source_id).unwrap();

    assert_eq!(
        output
            .tokens
            .iter()
            .map(|token| token.kind)
            .collect::<Vec<_>>(),
        [
            TokenKind::Identifier,
            TokenKind::LeftBracket,
            TokenKind::RightBracket,
            TokenKind::LeftBracket,
            I64_LITERAL,
            TokenKind::Colon,
            TokenKind::Minus,
            I64_LITERAL,
            TokenKind::RightBracket,
            TokenKind::Eof,
        ]
    );
    assert_eq!(
        output
            .tokens
            .iter()
            .map(|token| source.slice(token.span.range()).unwrap())
            .collect::<Vec<_>>(),
        ["T", "[", "]", "[", "4", ":", "-", "1", "]", ""]
    );
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
fn recognizes_boolean_type_and_literal_keywords() {
    let (_, _, output) = lex_text("bool true false boolean truthful");
    let kinds: Vec<_> = output.tokens.iter().map(|token| token.kind).collect();

    assert_eq!(
        kinds,
        vec![
            TokenKind::Bool,
            TokenKind::True,
            TokenKind::False,
            TokenKind::Identifier,
            TokenKind::Identifier,
            TokenKind::Eof,
        ]
    );
    assert!(!output.has_errors());
}

#[test]
fn recognizes_object_and_alias_keywords_without_reserving_prefixes() {
    let (_, _, output) =
        lex_text("class self mut ref reference ref_value init init_value object.field");
    let kinds: Vec<_> = output.tokens.iter().map(|token| token.kind).collect();

    assert_eq!(
        kinds,
        [
            TokenKind::Class,
            TokenKind::SelfValue,
            TokenKind::Mut,
            TokenKind::Ref,
            TokenKind::Identifier,
            TokenKind::Identifier,
            TokenKind::Identifier,
            TokenKind::Identifier,
            TokenKind::Identifier,
            TokenKind::Dot,
            TokenKind::Identifier,
            TokenKind::Eof,
        ]
    );
    assert!(!output.has_errors());
}

#[test]
fn leaves_language_feature_words_contextual() {
    let (_, _, output) = lex_text(
        "extends implements interface virtual override super is cast Obj copy shared new feature_value",
    );

    assert!(output.tokens[..output.tokens.len() - 1]
        .iter()
        .all(|token| token.kind == TokenKind::Identifier));
    assert_eq!(output.tokens.last().unwrap().kind, TokenKind::Eof);
    assert!(!output.has_errors());
}

#[test]
fn recognizes_u64_type_and_literal_without_reserving_identifier_prefixes() {
    let (_, _, output) = lex_text("u64 0u 18446744073709551615u u64_value unsigned");
    let kinds: Vec<_> = output.tokens.iter().map(|token| token.kind).collect();

    assert_eq!(
        kinds,
        [
            TokenKind::U64,
            U64_LITERAL,
            U64_LITERAL,
            TokenKind::Identifier,
            TokenKind::Identifier,
            TokenKind::Eof,
        ]
    );
    assert!(!output.has_errors());
}

#[test]
fn recognizes_u8_type_and_literal_without_reserving_identifier_prefixes() {
    let (_, _, output) = lex_text("u8 0u8 255u8 u8_value");
    let kinds: Vec<_> = output.tokens.iter().map(|token| token.kind).collect();

    assert_eq!(
        kinds,
        [
            TokenKind::U8,
            U8_LITERAL,
            U8_LITERAL,
            TokenKind::Identifier,
            TokenKind::Eof,
        ]
    );
    assert!(!output.has_errors());
}

#[test]
fn recognizes_f64_type_and_decimal_literal_forms() {
    let (_, _, output) = lex_text("f64 0.0 1.5 2e3 6.25e-1 f64_value");
    let kinds: Vec<_> = output.tokens.iter().map(|token| token.kind).collect();

    assert_eq!(
        kinds,
        [
            TokenKind::F64,
            F64_LITERAL,
            F64_LITERAL,
            F64_LITERAL,
            F64_LITERAL,
            TokenKind::Identifier,
            TokenKind::Eof,
        ]
    );
    assert!(!output.has_errors());
}

#[test]
fn recognizes_conditional_keywords_without_reserving_prefixes() {
    let (_, _, output) = lex_text("if elif else iffy elseif");
    let kinds: Vec<_> = output.tokens.iter().map(|token| token.kind).collect();

    assert_eq!(
        kinds,
        vec![
            TokenKind::If,
            TokenKind::Elif,
            TokenKind::Else,
            TokenKind::Identifier,
            TokenKind::Identifier,
            TokenKind::Eof,
        ]
    );
    assert!(!output.has_errors());
}

#[test]
fn reserves_loop_words_together_without_reserving_prefixes() {
    let (sources, source_id, output) =
        lex_text("while break continue while_value breaker continued");
    let source = sources.get(source_id).unwrap();

    assert_eq!(
        output
            .tokens
            .iter()
            .map(|token| token.kind)
            .collect::<Vec<_>>(),
        [
            TokenKind::While,
            TokenKind::Break,
            TokenKind::Continue,
            TokenKind::Identifier,
            TokenKind::Identifier,
            TokenKind::Identifier,
            TokenKind::Eof,
        ]
    );
    assert_eq!(
        output
            .tokens
            .iter()
            .map(|token| source.slice(token.span.range()).unwrap())
            .collect::<Vec<_>>(),
        [
            "while",
            "break",
            "continue",
            "while_value",
            "breaker",
            "continued",
            "",
        ]
    );
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
            I64_LITERAL,
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
    let (sources, source_id, output) = lex_text("12abc 1_000 12. 0xff");
    let source = sources.get(source_id).unwrap();
    let invalid_lexemes: Vec<_> = output
        .tokens
        .iter()
        .filter(|token| token.kind == TokenKind::Invalid)
        .map(|token| source.slice(token.span.range()).unwrap())
        .collect();

    assert_eq!(invalid_lexemes, vec!["12abc", "1_000", "12.", "0xff"]);
    assert_eq!(output.diagnostics.len(), 4);
    assert!(output
        .diagnostics
        .iter()
        .all(|diagnostic| diagnostic.code == MALFORMED_INTEGER_LITERAL));
}

#[test]
fn malformed_f64_spellings_recover_as_complete_tokens() {
    let (sources, source_id, output) = lex_text(".5 1. 1.2.3 1.0f64 return");
    let source = sources.get(source_id).unwrap();
    let spellings: Vec<_> = output
        .tokens
        .iter()
        .filter(|token| token.kind == TokenKind::Invalid)
        .map(|token| source.slice(token.span.range()).unwrap())
        .collect();

    assert_eq!(spellings, [".5", "1.", "1.2.3", "1.0f64"]);
    assert_eq!(output.diagnostics.len(), spellings.len());
    assert!(output
        .diagnostics
        .iter()
        .all(|diagnostic| diagnostic.code == MALFORMED_INTEGER_LITERAL));
    assert_eq!(output.tokens[4].kind, TokenKind::Return);
}

#[test]
fn malformed_exponents_are_recovered_without_splitting_their_sign() {
    let (sources, source_id, output) = lex_text("1e+ 2E-foo + 3");
    let source = sources.get(source_id).unwrap();
    let invalid: Vec<_> = output
        .tokens
        .iter()
        .filter(|token| token.kind == TokenKind::Invalid)
        .map(|token| source.slice(token.span.range()).unwrap())
        .collect();

    assert_eq!(invalid, ["1e+", "2E-foo"]);
    assert_eq!(output.tokens[2].kind, TokenKind::Plus);
    assert_eq!(output.tokens[3].kind, I64_LITERAL);
}

#[test]
fn integer_range_is_deliberately_not_checked_by_the_lexer() {
    let (_, _, output) = lex_text("9223372036854775808");

    assert_eq!(output.tokens[0].kind, I64_LITERAL);
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
fn double_colon_is_one_token_with_its_exact_span() {
    let (sources, source_id, output) = lex_text("std::Str");
    let source = sources.get(source_id).unwrap();

    assert_eq!(
        output
            .tokens
            .iter()
            .map(|token| token.kind)
            .collect::<Vec<_>>(),
        vec![
            TokenKind::Identifier,
            TokenKind::DoubleColon,
            TokenKind::Identifier,
            TokenKind::Eof,
        ]
    );
    assert_eq!(
        dump_tokens(source, &output.tokens),
        concat!(
            "IDENTIFIER 1:1..1:4 \"std\"\n",
            "DOUBLE_COLON 1:4..1:6 \"::\"\n",
            "IDENTIFIER 1:6..1:9 \"Str\"\n",
            "EOF 1:9..1:9 \"\"\n",
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
