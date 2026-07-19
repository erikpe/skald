//! Source text to token-stream lowering for the first vertical slice.
//!
//! The lexer accepts the deliberately narrow token set documented in
//! `grammar/README.md`. It recovers after invalid characters and malformed
//! decimal spellings, returning tokens and structured diagnostics together.

use std::fmt;

use crate::{
    diagnostics::{Diagnostic, Diagnostics},
    source::{SourceFile, Span},
};

pub const UNEXPECTED_CHARACTER: &str = "LEX001";
pub const MALFORMED_INTEGER_LITERAL: &str = "LEX002";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TokenKind {
    Fn,
    Var,
    Return,
    I64,
    Identifier,
    IntegerLiteral,
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    Comma,
    Colon,
    Semicolon,
    Arrow,
    Plus,
    Minus,
    Star,
    Equal,
    Invalid,
    Eof,
}

impl TokenKind {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Fn => "FN",
            Self::Var => "VAR",
            Self::Return => "RETURN",
            Self::I64 => "I64",
            Self::Identifier => "IDENTIFIER",
            Self::IntegerLiteral => "INTEGER_LITERAL",
            Self::LeftParen => "LEFT_PAREN",
            Self::RightParen => "RIGHT_PAREN",
            Self::LeftBrace => "LEFT_BRACE",
            Self::RightBrace => "RIGHT_BRACE",
            Self::Comma => "COMMA",
            Self::Colon => "COLON",
            Self::Semicolon => "SEMICOLON",
            Self::Arrow => "ARROW",
            Self::Plus => "PLUS",
            Self::Minus => "MINUS",
            Self::Star => "STAR",
            Self::Equal => "EQUAL",
            Self::Invalid => "INVALID",
            Self::Eof => "EOF",
        }
    }
}

impl fmt::Display for TokenKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.name())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

#[derive(Debug)]
pub struct LexOutput {
    pub tokens: Vec<Token>,
    pub diagnostics: Diagnostics,
}

impl LexOutput {
    pub fn has_errors(&self) -> bool {
        self.diagnostics.has_errors()
    }
}

pub fn lex(source: &SourceFile) -> LexOutput {
    Lexer::new(source).lex()
}

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
        dump.push('"');
        for character in lexeme.chars() {
            dump.extend(character.escape_default());
        }
        dump.push('"');
        dump.push('\n');
    }

    dump
}

struct Lexer<'source> {
    source: &'source SourceFile,
    offset: usize,
    tokens: Vec<Token>,
    diagnostics: Diagnostics,
}

impl<'source> Lexer<'source> {
    fn new(source: &'source SourceFile) -> Self {
        Self {
            source,
            offset: 0,
            tokens: Vec::new(),
            diagnostics: Diagnostics::new(),
        }
    }

    fn lex(mut self) -> LexOutput {
        while !self.at_end() {
            self.skip_trivia();
            if self.at_end() {
                break;
            }

            let start = self.offset;
            let character = self.peek().expect("checked for end of source");

            if is_identifier_start(character) {
                self.lex_identifier(start);
            } else if character.is_ascii_digit() {
                self.lex_integer(start);
            } else {
                self.lex_punctuation_or_invalid(start, character);
            }
        }

        self.tokens.push(Token {
            kind: TokenKind::Eof,
            span: Span::empty(self.source.id(), self.source.len()),
        });

        LexOutput {
            tokens: self.tokens,
            diagnostics: self.diagnostics,
        }
    }

    fn skip_trivia(&mut self) {
        loop {
            while self.peek().is_some_and(is_ascii_whitespace) {
                self.advance();
            }

            if self.remaining().starts_with("//") {
                self.advance();
                self.advance();
                while self.peek().is_some_and(|character| character != '\n') {
                    self.advance();
                }
                continue;
            }

            break;
        }
    }

    fn lex_identifier(&mut self, start: usize) {
        self.advance();
        while self.peek().is_some_and(is_identifier_continue) {
            self.advance();
        }

        let text = &self.source.text()[start..self.offset];
        let kind = match text {
            "fn" => TokenKind::Fn,
            "var" => TokenKind::Var,
            "return" => TokenKind::Return,
            "i64" => TokenKind::I64,
            _ => TokenKind::Identifier,
        };
        self.push_token(kind, start);
    }

    fn lex_integer(&mut self, start: usize) {
        self.advance();
        while self
            .peek()
            .is_some_and(|character| character.is_ascii_digit())
        {
            self.advance();
        }

        if self.peek().is_some_and(is_malformed_number_continue) {
            while self.peek().is_some_and(is_malformed_number_continue) {
                self.advance();
            }
            let span = self.span(start, self.offset);
            let spelling = &self.source.text()[start..self.offset];
            self.tokens.push(Token {
                kind: TokenKind::Invalid,
                span,
            });
            self.diagnostics.push(
                Diagnostic::error(
                    MALFORMED_INTEGER_LITERAL,
                    format!("malformed decimal integer literal `{spelling}`"),
                )
                .with_primary_label(span, "expected decimal digits only")
                .with_note("integer range checking occurs during type checking"),
            );
            return;
        }

        self.push_token(TokenKind::IntegerLiteral, start);
    }

    fn lex_punctuation_or_invalid(&mut self, start: usize, character: char) {
        if self.remaining().starts_with("->") {
            self.advance();
            self.advance();
            self.push_token(TokenKind::Arrow, start);
            return;
        }

        let kind = match character {
            '(' => TokenKind::LeftParen,
            ')' => TokenKind::RightParen,
            '{' => TokenKind::LeftBrace,
            '}' => TokenKind::RightBrace,
            ',' => TokenKind::Comma,
            ':' => TokenKind::Colon,
            ';' => TokenKind::Semicolon,
            '+' => TokenKind::Plus,
            '-' => TokenKind::Minus,
            '*' => TokenKind::Star,
            '=' => TokenKind::Equal,
            _ => {
                self.advance();
                let span = self.span(start, self.offset);
                let escaped: String = character.escape_default().collect();
                self.tokens.push(Token {
                    kind: TokenKind::Invalid,
                    span,
                });
                self.diagnostics.push(
                    Diagnostic::error(
                        UNEXPECTED_CHARACTER,
                        format!("unexpected character `{escaped}`"),
                    )
                    .with_primary_label(span, "not valid in the M1 grammar"),
                );
                return;
            }
        };

        self.advance();
        self.push_token(kind, start);
    }

    fn push_token(&mut self, kind: TokenKind, start: usize) {
        self.tokens.push(Token {
            kind,
            span: self.span(start, self.offset),
        });
    }

    fn span(&self, start: usize, end: usize) -> Span {
        self.source
            .span(start, end)
            .expect("lexer offsets must be valid UTF-8 boundaries")
    }

    fn at_end(&self) -> bool {
        self.offset == self.source.len()
    }

    fn remaining(&self) -> &'source str {
        &self.source.text()[self.offset..]
    }

    fn peek(&self) -> Option<char> {
        self.remaining().chars().next()
    }

    fn advance(&mut self) -> Option<char> {
        let character = self.peek()?;
        self.offset += character.len_utf8();
        Some(character)
    }
}

const fn is_ascii_whitespace(character: char) -> bool {
    matches!(character, ' ' | '\t' | '\r' | '\n')
}

const fn is_identifier_start(character: char) -> bool {
    character.is_ascii_alphabetic() || character == '_'
}

const fn is_identifier_continue(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_'
}

const fn is_malformed_number_continue(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_' || character == '.'
}

#[cfg(test)]
mod tests {
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
}
