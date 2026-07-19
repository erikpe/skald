use std::fmt;

use crate::source::Span;

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
