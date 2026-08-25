//! Source-shaped `for-in` header parsing and recovery boundaries.

use super::{declaration::TypeContext, *};

impl Parser<'_> {
    pub(super) fn parse_for_in(&mut self) -> Option<ForInStatement> {
        let for_token = self.advance();
        let left_paren = self.expect(TokenKind::LeftParen, "`(` after `for`");
        let binding = self.parse_for_in_binding();
        let (annotation, annotation_valid) = self.parse_for_in_annotation();
        let in_token = self.parse_for_in_delimiter();
        let iterable = self.parse_for_in_iterable();
        let right_paren = self.expect(TokenKind::RightParen, "`)` after the iterable expression");
        let body = self.parse_block();

        match (left_paren, binding, in_token, iterable, right_paren, body) {
            (
                Some(left_paren),
                Some(binding),
                Some(in_token),
                Some(iterable),
                Some(right_paren),
                Some(body),
            ) if annotation_valid => Some(ForInStatement {
                for_span: for_token.span,
                left_paren_span: left_paren.span,
                binding,
                annotation,
                in_span: in_token.span,
                iterable,
                right_paren_span: right_paren.span,
                span: self.cover(for_token.span, body.span),
                body,
            }),
            _ => None,
        }
    }

    fn parse_for_in_binding(&mut self) -> Option<Name> {
        if self.at_any(&[
            TokenKind::Colon,
            TokenKind::RightParen,
            TokenKind::LeftBrace,
            TokenKind::RightBrace,
            TokenKind::Eof,
        ]) {
            self.report(
                EXPECTED_TOKEN,
                "expected an item binding after `for (`",
                self.peek().span,
                "expected an identifier here",
            );
            return None;
        }
        self.parse_name("an item binding after `for (`")
    }

    fn parse_for_in_annotation(&mut self) -> (Option<ForInTypeAnnotation>, bool) {
        let Some(colon) = self.consume(TokenKind::Colon) else {
            return (None, true);
        };
        let Some(type_syntax) = self.parse_type(
            TypeContext::LocalValue,
            format!(
                "expected the item type {}, a class name, or a shared object type",
                format_type_list(STORED_TYPE_NAMES)
            ),
        ) else {
            self.synchronize_for_in_header();
            return (None, false);
        };
        (
            Some(ForInTypeAnnotation {
                span: self.cover(colon.span, type_syntax.span),
                colon_span: colon.span,
                type_syntax,
            }),
            true,
        )
    }

    fn parse_for_in_delimiter(&mut self) -> Option<Token> {
        if self.at_contextual("in") {
            return Some(self.advance());
        }
        self.report(
            EXPECTED_TOKEN,
            "expected contextual `in` after the item binding",
            self.peek().span,
            "expected `in` here",
        );
        None
    }

    fn parse_for_in_iterable(&mut self) -> Option<Expression> {
        if self.at_any(&[
            TokenKind::RightParen,
            TokenKind::LeftBrace,
            TokenKind::RightBrace,
            TokenKind::Eof,
        ]) {
            self.report(
                EXPECTED_EXPRESSION,
                "expected an iterable expression after `in`",
                self.peek().span,
                "expected an expression here",
            );
            return None;
        }
        self.parse_expression()
    }
}
