//! Grammar-aware synchronization and expression-start classification.

use crate::literal::NumericLiteralKind;

use super::*;

impl Parser<'_> {
    pub(super) fn synchronize_declaration(&mut self) {
        while !self.at_any(&[TokenKind::Fn, TokenKind::Extern, TokenKind::Eof]) {
            self.advance();
        }
        self.recovering_from_excessive_nesting = false;
    }

    /// Discards the rest of the over-deep declaration without recursively
    /// inspecting its delimiters. `fn` and `extern` cannot begin valid nested
    /// syntax in the implemented grammar, so they are reliable restart points.
    pub(super) fn recover_from_excessive_nesting(&mut self) {
        while !self.at_any(&[TokenKind::Fn, TokenKind::Extern, TokenKind::Eof]) {
            self.advance();
        }
    }

    pub(super) fn synchronize_parameter(&mut self) {
        while !self.at_any(&[
            TokenKind::Comma,
            TokenKind::RightParen,
            TokenKind::Arrow,
            TokenKind::LeftBrace,
            TokenKind::Semicolon,
            TokenKind::Fn,
            TokenKind::Extern,
            TokenKind::Eof,
        ]) {
            self.advance();
        }
    }

    pub(super) fn synchronize_statement(&mut self) {
        while !self.at(TokenKind::Eof) {
            if self.consume(TokenKind::Semicolon).is_some() {
                return;
            }
            if self.at_any(&[
                TokenKind::Var,
                TokenKind::Return,
                TokenKind::If,
                TokenKind::Elif,
                TokenKind::Else,
                TokenKind::Identifier,
                TokenKind::NumericLiteral(NumericLiteralKind::I64),
                TokenKind::NumericLiteral(NumericLiteralKind::U64),
                TokenKind::NumericLiteral(NumericLiteralKind::U8),
                TokenKind::NumericLiteral(NumericLiteralKind::F64),
                TokenKind::True,
                TokenKind::False,
                TokenKind::Minus,
                TokenKind::LeftParen,
                TokenKind::LeftBrace,
                TokenKind::RightBrace,
                TokenKind::Fn,
                TokenKind::Extern,
            ]) {
                return;
            }
            self.advance();
        }
    }

    pub(super) fn synchronize_argument(&mut self) {
        while !self.at_any(&[
            TokenKind::Comma,
            TokenKind::RightParen,
            TokenKind::Semicolon,
            TokenKind::RightBrace,
            TokenKind::Eof,
        ]) {
            self.advance();
        }
    }

    pub(super) fn starts_expression(&self) -> bool {
        self.at_any(&[
            TokenKind::Identifier,
            TokenKind::NumericLiteral(NumericLiteralKind::I64),
            TokenKind::NumericLiteral(NumericLiteralKind::U64),
            TokenKind::NumericLiteral(NumericLiteralKind::U8),
            TokenKind::NumericLiteral(NumericLiteralKind::F64),
            TokenKind::True,
            TokenKind::False,
            TokenKind::Minus,
            TokenKind::LeftParen,
        ])
    }
}
