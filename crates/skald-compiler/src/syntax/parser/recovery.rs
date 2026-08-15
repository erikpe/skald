//! Grammar-aware synchronization and expression-start classification.

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
    /// rebuilding its syntax. A nested `fn` token may belong to a function
    /// type, so recovery first crosses the current declaration's body (or its
    /// terminating semicolon) before treating declaration keywords as restart
    /// points.
    pub(super) fn recover_from_excessive_nesting(&mut self) {
        if self.brace_depth > 0 {
            let mut braces_to_close = self.brace_depth;
            while braces_to_close > 0 && !self.at(TokenKind::Eof) {
                match self.advance().kind {
                    TokenKind::LeftBrace => braces_to_close += 1,
                    TokenKind::RightBrace => braces_to_close -= 1,
                    _ => {}
                }
            }
        } else {
            while !self.at_any(&[TokenKind::LeftBrace, TokenKind::Semicolon, TokenKind::Eof]) {
                self.advance();
            }
            if self.consume(TokenKind::LeftBrace).is_some() {
                let mut braces_to_close = 1usize;
                while braces_to_close > 0 && !self.at(TokenKind::Eof) {
                    match self.advance().kind {
                        TokenKind::LeftBrace => braces_to_close += 1,
                        TokenKind::RightBrace => braces_to_close -= 1,
                        _ => {}
                    }
                }
            } else {
                self.consume(TokenKind::Semicolon);
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
            if matches!(self.peek().kind, TokenKind::NumericLiteral(_))
                || self.at_any(&[
                    TokenKind::Var,
                    TokenKind::Return,
                    TokenKind::If,
                    TokenKind::Elif,
                    TokenKind::Else,
                    TokenKind::While,
                    TokenKind::Break,
                    TokenKind::Continue,
                    TokenKind::Identifier,
                    TokenKind::I64,
                    TokenKind::U64,
                    TokenKind::U8,
                    TokenKind::F64,
                    TokenKind::Bool,
                    TokenKind::Unit,
                    TokenKind::SelfValue,
                    TokenKind::ByteLiteral,
                    TokenKind::StringLiteral,
                    TokenKind::True,
                    TokenKind::False,
                    TokenKind::Minus,
                    TokenKind::LeftParen,
                    TokenKind::LeftBrace,
                    TokenKind::RightBrace,
                    TokenKind::Fn,
                    TokenKind::Extern,
                    TokenKind::Class,
                ])
            {
                return;
            }
            self.advance();
        }
    }

    pub(super) fn synchronize_argument(&mut self) {
        while !self.at_any(&[
            TokenKind::Comma,
            TokenKind::RightParen,
            TokenKind::Colon,
            TokenKind::RightBracket,
            TokenKind::Semicolon,
            TokenKind::RightBrace,
            TokenKind::Eof,
        ]) {
            self.advance();
        }
    }

    pub(super) fn starts_expression(&self) -> bool {
        Self::token_starts_expression(self.peek().kind)
    }

    pub(super) fn starts_expression_ahead(&self, distance: usize) -> bool {
        Self::token_starts_expression(self.peek_ahead(distance).kind)
    }

    const fn token_starts_expression(kind: TokenKind) -> bool {
        matches!(
            kind,
            TokenKind::Identifier
                | TokenKind::I64
                | TokenKind::U64
                | TokenKind::U8
                | TokenKind::F64
                | TokenKind::Bool
                | TokenKind::Unit
                | TokenKind::SelfValue
                | TokenKind::NumericLiteral(_)
                | TokenKind::ByteLiteral
                | TokenKind::StringLiteral
                | TokenKind::True
                | TokenKind::False
                | TokenKind::None
                | TokenKind::Minus
                | TokenKind::Star
                | TokenKind::Bang
                | TokenKind::Tilde
                | TokenKind::LeftParen
        )
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
