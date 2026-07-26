//! Array construction and postfix projection source shapes.

use super::{declaration::TypeContext, *};

impl Parser<'_> {
    pub(super) fn finish_array_projection(&mut self, receiver: Expression) -> Option<Expression> {
        let operator = if let Some(arrow) = self.consume(TokenKind::Arrow) {
            let left_bracket = self.expect(
                TokenKind::LeftBracket,
                "`[` after the shared projection arrow",
            )?;
            ArrayProjectionOperator::Shared {
                arrow_span: arrow.span,
                left_bracket_span: left_bracket.span,
            }
        } else {
            let left_bracket = self.advance();
            debug_assert_eq!(left_bracket.kind, TokenKind::LeftBracket);
            ArrayProjectionOperator::Ordinary {
                left_bracket_span: left_bracket.span,
            }
        };

        let bounds = if let Some(colon) = self.consume(TokenKind::Colon) {
            let end = (!self.at(TokenKind::RightBracket))
                .then(|| self.parse_expression())
                .flatten()
                .map(Box::new);
            ArrayProjectionBounds::Slice {
                start: None,
                colon_span: colon.span,
                end,
            }
        } else {
            if self.at(TokenKind::RightBracket) {
                self.report(
                    EXPECTED_EXPRESSION,
                    "expected an array index or slice",
                    self.peek().span,
                    "use `[:]` for a full slice",
                );
                self.advance();
                return None;
            }
            let start = self.parse_expression()?;
            if let Some(colon) = self.consume(TokenKind::Colon) {
                let end = (!self.at(TokenKind::RightBracket))
                    .then(|| self.parse_expression())
                    .flatten()
                    .map(Box::new);
                ArrayProjectionBounds::Slice {
                    start: Some(Box::new(start)),
                    colon_span: colon.span,
                    end,
                }
            } else {
                ArrayProjectionBounds::Index(Box::new(start))
            }
        };

        let right_bracket =
            self.expect(TokenKind::RightBracket, "`]` after the array projection")?;
        let span = self.cover(receiver.span(), right_bracket.span);
        Some(Expression::ArrayProjection(Box::new(ArrayProjectionExpr {
            receiver: Box::new(receiver),
            operator,
            bounds,
            right_bracket_span: right_bracket.span,
            span,
        })))
    }

    pub(super) fn starts_array_construction(&self, after_new: bool) -> bool {
        let start = usize::from(after_new);
        if !after_new
            && self.peek_ahead(start).kind == TokenKind::LeftParen
            && !token_type_starts_array_element(self.peek_ahead(start + 1).kind)
        {
            return false;
        }
        self.scan_type_shape(start, 0)
            .is_some_and(|(end, contains_array)| {
                contains_array && self.peek_ahead(end).kind == TokenKind::LeftParen
            })
    }

    fn scan_type_shape(&self, distance: usize, depth: usize) -> Option<(usize, bool)> {
        if depth >= MAX_SYNTAX_NESTING {
            return None;
        }
        let token = self.peek_ahead(distance);
        let (mut end, mut contains_array) =
            if token.kind == TokenKind::Identifier && self.lexeme(token) == "shared" {
                let mut target = distance + 1;
                if self.peek_ahead(target).kind == TokenKind::Question {
                    target += 1;
                }
                let (end, contains_array) = self.scan_type_shape(target, depth + 1)?;
                (end, contains_array)
            } else if token.kind == TokenKind::LeftParen {
                let (inner_end, contains_array) = self.scan_type_shape(distance + 1, depth + 1)?;
                if self.peek_ahead(inner_end).kind != TokenKind::RightParen {
                    return None;
                }
                (inner_end + 1, contains_array)
            } else if token_type_starts_array_element(token.kind) {
                let mut end = distance + 1;
                if self.peek_ahead(end).kind == TokenKind::Question {
                    end += 1;
                }
                (end, false)
            } else {
                return None;
            };

        while self.peek_ahead(end).kind == TokenKind::LeftBracket
            && self.peek_ahead(end + 1).kind == TokenKind::RightBracket
        {
            contains_array = true;
            end += 2;
        }
        Some((end, contains_array))
    }

    pub(super) fn parse_array_construction(&mut self, shared: bool) -> Option<Expression> {
        let new_span = shared.then(|| self.advance().span);
        let array_type = self.parse_type(
            TypeContext::ArrayElement,
            "expected an array type in the construction",
        )?;
        if !matches!(array_type.kind, TypeKind::Array { .. }) {
            self.report(
                EXPECTED_TOKEN,
                "array construction requires an inline array target",
                array_type.span,
                "put outer shared ownership in `new`, not in the constructed type",
            );
            return None;
        }

        let left_paren = self.expect(TokenKind::LeftParen, "`(` after the array type")?;
        let arguments = if let Some(right_paren) = self.consume(TokenKind::RightParen) {
            ArrayConstructionArguments::Empty {
                left_paren_span: left_paren.span,
                right_paren_span: right_paren.span,
            }
        } else if self.at_contextual("copy") && self.starts_expression_ahead(1) {
            let copy = self.advance();
            let source = self.parse_expression()?;
            let right_paren = self.expect(
                TokenKind::RightParen,
                "`)` after the explicit array copy source",
            )?;
            ArrayConstructionArguments::Copy {
                left_paren_span: left_paren.span,
                copy_span: copy.span,
                source: Box::new(source),
                right_paren_span: right_paren.span,
            }
        } else {
            let length = self.parse_expression()?;
            let right_paren = self.expect(TokenKind::RightParen, "`)` after the array length")?;
            ArrayConstructionArguments::Length {
                left_paren_span: left_paren.span,
                length: Box::new(length),
                right_paren_span: right_paren.span,
            }
        };
        let end_span = match &arguments {
            ArrayConstructionArguments::Empty {
                right_paren_span, ..
            }
            | ArrayConstructionArguments::Length {
                right_paren_span, ..
            }
            | ArrayConstructionArguments::Copy {
                right_paren_span, ..
            } => *right_paren_span,
        };
        let start_span = new_span.unwrap_or(array_type.span);
        Some(Expression::ArrayConstruction(Box::new(
            ArrayConstructionExpr {
                new_span,
                array_type,
                arguments,
                span: self.cover(start_span, end_span),
            },
        )))
    }
}

const fn token_type_starts_array_element(kind: TokenKind) -> bool {
    matches!(
        kind,
        TokenKind::Identifier
            | TokenKind::I64
            | TokenKind::U64
            | TokenKind::U8
            | TokenKind::F64
            | TokenKind::Bool
            | TokenKind::Unit
    )
}
