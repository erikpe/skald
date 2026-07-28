use std::fmt;

use crate::{literal::NumericLiteralKind, source::Span};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TokenKind {
    Class,
    SelfValue,
    Mut,
    Ref,
    Fn,
    Extern,
    Var,
    Return,
    I64,
    U64,
    U8,
    F64,
    Bool,
    True,
    False,
    If,
    Elif,
    Else,
    Unit,
    None,
    Identifier,
    NumericLiteral(NumericLiteralKind),
    StringLiteral,
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    LeftBracket,
    RightBracket,
    Comma,
    Colon,
    DoubleColon,
    Semicolon,
    Arrow,
    Plus,
    Minus,
    Star,
    Equal,
    Dot,
    Question,
    Bang,
    Invalid,
    Eof,
}

impl TokenKind {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Class => "CLASS",
            Self::SelfValue => "SELF",
            Self::Mut => "MUT",
            Self::Ref => "REF",
            Self::Fn => "FN",
            Self::Extern => "EXTERN",
            Self::Var => "VAR",
            Self::Return => "RETURN",
            Self::I64 => "I64",
            Self::U64 => "U64",
            Self::U8 => "U8",
            Self::F64 => "F64",
            Self::Bool => "BOOL",
            Self::True => "TRUE",
            Self::False => "FALSE",
            Self::If => "IF",
            Self::Elif => "ELIF",
            Self::Else => "ELSE",
            Self::Unit => "UNIT",
            Self::None => "NONE",
            Self::Identifier => "IDENTIFIER",
            Self::NumericLiteral(NumericLiteralKind::I64) => "INTEGER_LITERAL",
            Self::NumericLiteral(NumericLiteralKind::U64) => "U64_LITERAL",
            Self::NumericLiteral(NumericLiteralKind::U8) => "U8_LITERAL",
            Self::NumericLiteral(NumericLiteralKind::F64) => "F64_LITERAL",
            Self::StringLiteral => "STRING_LITERAL",
            Self::LeftParen => "LEFT_PAREN",
            Self::RightParen => "RIGHT_PAREN",
            Self::LeftBrace => "LEFT_BRACE",
            Self::RightBrace => "RIGHT_BRACE",
            Self::LeftBracket => "LEFT_BRACKET",
            Self::RightBracket => "RIGHT_BRACKET",
            Self::Comma => "COMMA",
            Self::Colon => "COLON",
            Self::DoubleColon => "DOUBLE_COLON",
            Self::Semicolon => "SEMICOLON",
            Self::Arrow => "ARROW",
            Self::Plus => "PLUS",
            Self::Minus => "MINUS",
            Self::Star => "STAR",
            Self::Equal => "EQUAL",
            Self::Dot => "DOT",
            Self::Question => "QUESTION",
            Self::Bang => "BANG",
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
