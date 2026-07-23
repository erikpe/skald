//! Grammar-aware synchronization and expression-start classification.

use crate::literal::NumericLiteralKind;

use super::*;

impl Parser<'_> {
    pub(super) fn synchronize_declaration(&mut self) {
        while !self.at_any(&[
            TokenKind::Fn,
            TokenKind::Extern,
            TokenKind::Class,
            TokenKind::Eof,
        ]) && !self.at_contextual("interface")
        {
            self.advance();
        }
        self.recovering_from_excessive_nesting = false;
    }

    /// Discards the rest of an over-deep declaration without recursively
    /// rebuilding its syntax. Inside a class, brace accounting first skips the
    /// complete class because `fn` can introduce either a method or the next
    /// top-level declaration. File-scope keywords are then reliable restart
    /// points.
    pub(super) fn recover_from_excessive_nesting(&mut self) {
        if self.class_depth > 0 {
            let mut braces_to_close = self.brace_depth;
            while braces_to_close > 0 && !self.at(TokenKind::Eof) {
                match self.advance().kind {
                    TokenKind::LeftBrace => braces_to_close += 1,
                    TokenKind::RightBrace => braces_to_close -= 1,
                    _ => {}
                }
            }
        }
        while !self.at_any(&[
            TokenKind::Fn,
            TokenKind::Extern,
            TokenKind::Class,
            TokenKind::Eof,
        ]) {
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
            TokenKind::Class,
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
                TokenKind::SelfValue,
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
                TokenKind::Class,
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
            TokenKind::SelfValue,
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

    pub(super) fn synchronize_class_member(&mut self) {
        let mut brace_depth = 0usize;
        while !self.at(TokenKind::Eof) {
            match self.peek().kind {
                TokenKind::LeftBrace => {
                    brace_depth += 1;
                    self.advance();
                }
                TokenKind::RightBrace if brace_depth == 0 => return,
                TokenKind::RightBrace => {
                    brace_depth -= 1;
                    self.advance();
                    if brace_depth == 0 {
                        return;
                    }
                }
                TokenKind::Semicolon if brace_depth == 0 => {
                    self.advance();
                    return;
                }
                _ => {
                    self.advance();
                }
            }
        }
    }
}
