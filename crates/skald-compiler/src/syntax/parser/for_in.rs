//! Source-shaped `for-in` header parsing and recovery boundaries.

use super::{declaration::TypeContext, *};

impl Parser<'_> {
    pub(super) fn parse_for_in(&mut self) -> Option<ForInStatement> {
        let for_token = self.advance();
        let left_paren = self.expect(TokenKind::LeftParen, "`(` after `for`");
        let binding = self.parse_for_in_binding();
        let (annotation, annotation_valid) = self.parse_for_in_annotation();
        let in_token = self.parse_for_in_delimiter();
        let source = self.parse_for_in_source();
        let right_paren = self.expect(TokenKind::RightParen, "`)` after the iterable expression");
        let body = self.parse_block();

        match (left_paren, binding, in_token, source, right_paren, body) {
            (
                Some(left_paren),
                Some(binding),
                Some(in_token),
                Some(source),
                Some(right_paren),
                Some(body),
            ) if annotation_valid => Some(ForInStatement {
                for_span: for_token.span,
                left_paren_span: left_paren.span,
                binding,
                annotation,
                in_span: in_token.span,
                source,
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

    fn parse_for_in_source(&mut self) -> Option<ForInSource> {
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
        if self.at(TokenKind::DotDot) {
            self.reject_missing_for_range_lower();
            return None;
        }

        let lower = self.parse_for_range_endpoint()?;
        let Some(operator) = self.consume(TokenKind::DotDot) else {
            return Some(ForInSource::Iterable(lower));
        };
        if !self.starts_expression() || self.at(TokenKind::DotDot) {
            self.report(
                INVALID_RANGE_SYNTAX,
                "direct range sources require an upper endpoint",
                operator.span,
                "expected an expression after `..`",
            );
            self.consume_range_tail();
            return None;
        }
        let upper = self.parse_for_range_endpoint()?;
        if self.at(TokenKind::DotDot) {
            self.reject_for_range_chain();
            return None;
        }
        Some(ForInSource::Range(Box::new(ForRangeSource {
            span: self.cover(lower.span(), upper.span()),
            lower,
            operator_span: operator.span,
            upper,
        })))
    }

    #[cold]
    fn reject_missing_for_range_lower(&mut self) {
        let operator = self.advance();
        self.report(
            INVALID_RANGE_SYNTAX,
            "direct range sources require a lower endpoint",
            operator.span,
            "expected an expression before `..`",
        );
        self.consume_range_tail();
    }

    #[cold]
    fn reject_for_range_chain(&mut self) {
        let operator = self.advance();
        self.report(
            INVALID_RANGE_SYNTAX,
            "direct range operators cannot be chained",
            operator.span,
            "use one half-open range as the direct `for-in` source",
        );
        self.consume_range_tail();
    }

    #[cold]
    pub(super) fn reject_range_outside_for_source(&mut self) {
        let operator = self.advance();
        self.report(
            INVALID_RANGE_SYNTAX,
            "concise range syntax is allowed only as the direct `for-in` source",
            operator.span,
            "construct a `Range` value explicitly here",
        );
        self.consume_range_tail();
    }

    fn consume_range_tail(&mut self) {
        if self.starts_expression() && !self.at(TokenKind::DotDot) {
            let _ = self.parse_for_range_endpoint();
        }
        while self.consume(TokenKind::DotDot).is_some() {
            if self.starts_expression() && !self.at(TokenKind::DotDot) {
                let _ = self.parse_for_range_endpoint();
            }
        }
    }
}
