//! Source grammar shared by generic declarations and closed type applications.

use super::{
    declaration::{token_starts_type, TypeContext},
    *,
};

impl Parser<'_> {
    pub(super) fn parse_named_type(&mut self, message: &'static str) -> Option<NamedTypeSyntax> {
        let name = self.parse_name_path(message)?;
        let arguments = if self.at(TokenKind::Less) {
            Some(Box::new(self.parse_generic_argument_list()?))
        } else {
            None
        };
        let span = arguments
            .as_ref()
            .map_or(name.span, |arguments| self.cover(name.span, arguments.span));
        Some(NamedTypeSyntax {
            name,
            arguments,
            span,
        })
    }

    pub(super) fn parse_generic_parameter_list(&mut self) -> Option<GenericParameterList> {
        let left_angle = self.advance();
        debug_assert_eq!(left_angle.kind, TokenKind::Less);

        if self.at(TokenKind::Greater) {
            let right_angle = self.advance();
            self.report(
                INVALID_GENERIC_SYNTAX,
                "a generic declaration must declare at least one type parameter",
                self.cover(left_angle.span, right_angle.span),
                "remove the empty list or add a parameter name",
            );
            return None;
        }

        let mut parameters = Vec::new();
        let mut comma_spans = Vec::new();
        loop {
            let Some(parameter) = self.parse_name("expected a type parameter name") else {
                self.recover_angle_list();
                return None;
            };
            parameters.push(parameter);

            if let Some(comma) = self.consume(TokenKind::Comma) {
                comma_spans.push(comma.span);
                if self.at(TokenKind::Greater) {
                    let right_angle = self.advance();
                    self.report(
                        INVALID_GENERIC_SYNTAX,
                        "generic parameter lists do not allow a trailing comma",
                        comma.span,
                        "remove this comma",
                    );
                    let span = self.cover(left_angle.span, right_angle.span);
                    return Some(GenericParameterList {
                        left_angle_span: left_angle.span,
                        parameters,
                        comma_spans,
                        right_angle_span: right_angle.span,
                        span,
                    });
                }
                continue;
            }
            if self.at(TokenKind::Identifier) {
                self.report(
                    INVALID_GENERIC_SYNTAX,
                    "expected `,` between generic type parameters",
                    self.peek().span,
                    "insert `,` before this parameter",
                );
                continue;
            }
            break;
        }

        let right_angle = self.expect(TokenKind::Greater, "`>` after type parameters")?;
        let span = self.cover(left_angle.span, right_angle.span);
        Some(GenericParameterList {
            left_angle_span: left_angle.span,
            parameters,
            comma_spans,
            right_angle_span: right_angle.span,
            span,
        })
    }

    pub(super) fn parse_generic_where_clause(&mut self) -> Option<GenericWhereClause> {
        let where_token = self.advance();
        debug_assert_eq!(self.lexeme(where_token), "where");
        let mut requirements = Vec::new();
        let mut comma_spans = Vec::new();

        loop {
            let parameter = self.parse_name("expected a type parameter after `where`")?;
            let colon = self.expect(TokenKind::Colon, "`:` after the constrained parameter")?;
            let interface = self.parse_named_type("expected an interface name after `:`")?;
            let requirement_span = self.cover(parameter.span, interface.span);
            requirements.push(GenericRequirementSyntax {
                parameter,
                colon_span: colon.span,
                interface,
                span: requirement_span,
            });

            if let Some(comma) = self.consume(TokenKind::Comma) {
                comma_spans.push(comma.span);
                if self.at(TokenKind::LeftBrace) {
                    self.report(
                        INVALID_GENERIC_SYNTAX,
                        "generic `where` clauses do not allow a trailing comma",
                        comma.span,
                        "remove this comma",
                    );
                    break;
                }
                continue;
            }
            if self.at(TokenKind::Identifier)
                && !self.at_contextual("extends")
                && !self.at_contextual("implements")
                && !self.at_contextual("where")
            {
                self.report(
                    INVALID_GENERIC_SYNTAX,
                    "expected `,` between generic requirements",
                    self.peek().span,
                    "insert `,` before this requirement",
                );
                continue;
            }
            break;
        }

        let end = requirements
            .last()
            .map_or(where_token.span, |requirement| requirement.span);
        Some(GenericWhereClause {
            where_span: where_token.span,
            requirements,
            comma_spans,
            span: self.cover(where_token.span, end),
        })
    }

    fn parse_generic_argument_list(&mut self) -> Option<GenericArgumentList> {
        self.generic_argument_depth += 1;
        let result = self.parse_generic_argument_list_contents();
        self.generic_argument_depth -= 1;

        if self.generic_argument_depth == 0 {
            if let Some(extra_closer) = self.pending_generic_closer.take() {
                self.report(
                    INVALID_GENERIC_SYNTAX,
                    "unexpected `>` after generic type arguments",
                    extra_closer.span,
                    "remove this extra closer",
                );
            }
        }

        result
    }

    fn parse_generic_argument_list_contents(&mut self) -> Option<GenericArgumentList> {
        let left_angle = self.advance();
        debug_assert_eq!(left_angle.kind, TokenKind::Less);
        if self.at_generic_argument_close() {
            let right_angle = self.consume_generic_argument_close()?;
            self.report(
                INVALID_GENERIC_SYNTAX,
                "a generic application must supply at least one type argument",
                self.cover(left_angle.span, right_angle.span),
                "add a type argument",
            );
            return None;
        }

        let mut arguments = Vec::new();
        let mut comma_spans = Vec::new();
        loop {
            let argument = self.with_syntax_nesting(left_angle.span, |parser| {
                parser.parse_type(
                    TypeContext::ArrayElement,
                    "expected a generic type argument",
                )
            })?;
            arguments.push(argument);

            // A closer split from `>>` belongs to this argument list before
            // any current token can act as its separator. The current comma,
            // when present, belongs to an enclosing generic, bound, or claim
            // list and must remain available to that parser.
            if self.pending_generic_closer.is_some() {
                break;
            }
            if let Some(comma) = self.consume(TokenKind::Comma) {
                comma_spans.push(comma.span);
                if self.at_generic_argument_close() {
                    let right_angle = self.consume_generic_argument_close()?;
                    self.report(
                        INVALID_GENERIC_SYNTAX,
                        "generic argument lists do not allow a trailing comma",
                        comma.span,
                        "remove this comma",
                    );
                    let span = self.cover(left_angle.span, right_angle.span);
                    return Some(GenericArgumentList {
                        left_angle_span: left_angle.span,
                        arguments,
                        comma_spans,
                        right_angle_span: right_angle.span,
                        span,
                    });
                }
                continue;
            }
            if self.at_generic_argument_close() {
                break;
            }
            if token_starts_type(self.peek().kind) {
                self.report(
                    INVALID_GENERIC_SYNTAX,
                    "expected `,` between generic type arguments",
                    self.peek().span,
                    "insert `,` before this type argument",
                );
                continue;
            }
            self.report(
                EXPECTED_TOKEN,
                "expected `>` after generic type arguments",
                self.peek().span,
                "close this generic argument list",
            );
            return None;
        }

        let right_angle = self.consume_generic_argument_close()?;
        let span = self.cover(left_angle.span, right_angle.span);
        Some(GenericArgumentList {
            left_angle_span: left_angle.span,
            arguments,
            comma_spans,
            right_angle_span: right_angle.span,
            span,
        })
    }

    fn at_generic_argument_close(&self) -> bool {
        self.pending_generic_closer.is_some()
            || matches!(self.peek().kind, TokenKind::Greater | TokenKind::ShiftRight)
    }

    fn consume_generic_argument_close(&mut self) -> Option<Token> {
        if let Some(token) = self.pending_generic_closer.take() {
            return Some(token);
        }
        if self.at(TokenKind::Greater) {
            return Some(self.advance());
        }
        if self.at(TokenKind::ShiftRight) {
            let combined = self.advance();
            let start = combined.span.range().start();
            let source_id = combined.span.source_id();
            let first = Token {
                kind: TokenKind::Greater,
                span: Span::new(
                    source_id,
                    crate::source::TextRange::new(start, start + 1)
                        .expect("split closer span is ordered"),
                ),
            };
            self.pending_generic_closer = Some(Token {
                kind: TokenKind::Greater,
                span: Span::new(
                    source_id,
                    crate::source::TextRange::new(start + 1, start + 2)
                        .expect("split closer span is ordered"),
                ),
            });
            return Some(first);
        }
        self.report(
            EXPECTED_TOKEN,
            "expected `>` after generic type arguments",
            self.peek().span,
            "close this generic argument list",
        );
        None
    }

    fn recover_angle_list(&mut self) {
        while !self.at_any(&[
            TokenKind::Greater,
            TokenKind::LeftBrace,
            TokenKind::Semicolon,
            TokenKind::Eof,
        ]) {
            self.advance();
        }
        self.consume(TokenKind::Greater);
    }
}
