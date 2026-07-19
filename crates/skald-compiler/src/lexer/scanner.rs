use crate::{
    diagnostics::{Diagnostic, Diagnostics},
    source::{SourceFile, Span},
};

use super::{Token, TokenKind};

pub const UNEXPECTED_CHARACTER: &str = "LEX001";
pub const MALFORMED_INTEGER_LITERAL: &str = "LEX002";

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
            "extern" => TokenKind::Extern,
            "var" => TokenKind::Var,
            "return" => TokenKind::Return,
            "i64" => TokenKind::I64,
            "unit" => TokenKind::Unit,
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
