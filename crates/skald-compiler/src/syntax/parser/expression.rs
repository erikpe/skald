//! Expression grammar, precedence, grouping, and calls.

use super::*;

impl Parser<'_> {
    pub(super) fn parse_expression(&mut self) -> Option<Expression> {
        let source = self.parse_additive()?;
        if !self.at_contextual("is") {
            return Some(source);
        }

        let is_token = self.advance();
        let target = self.parse_name("expected a class, interface, or `Obj` after `is`")?;
        let span = self.cover(source.span(), target.span);
        let expression = Expression::TypeTest(TypeTestExpr {
            source: Box::new(source),
            is_span: is_token.span,
            target,
            span,
        });
        if self.at_contextual("is") {
            let chained = self.advance();
            self.report(
                INVALID_TYPE_TEST,
                "type tests cannot be chained",
                chained.span,
                "group separate tests explicitly",
            );
            let _ = self.parse_name("expected a type after `is`");
            return None;
        }
        Some(expression)
    }

    fn parse_additive(&mut self) -> Option<Expression> {
        let mut expression = self.parse_multiplicative()?;

        while self.at_any(&[TokenKind::Plus, TokenKind::Minus]) {
            let operator = self.advance();
            let right = self.parse_multiplicative()?;
            let kind = match operator.kind {
                TokenKind::Plus => BinaryOperator::Add,
                TokenKind::Minus => BinaryOperator::Subtract,
                _ => unreachable!("additive parser accepted a non-additive operator"),
            };
            let span = self.cover(expression.span(), right.span());
            expression = Expression::Binary(BinaryExpr {
                left: Box::new(expression),
                operator: kind,
                operator_span: operator.span,
                right: Box::new(right),
                span,
            });
        }

        Some(expression)
    }

    fn parse_multiplicative(&mut self) -> Option<Expression> {
        let mut expression = self.parse_unary()?;

        while self.at(TokenKind::Star) {
            let operator = self.advance();
            let right = self.parse_unary()?;
            let span = self.cover(expression.span(), right.span());
            expression = Expression::Binary(BinaryExpr {
                left: Box::new(expression),
                operator: BinaryOperator::Multiply,
                operator_span: operator.span,
                right: Box::new(right),
                span,
            });
        }

        Some(expression)
    }

    fn parse_unary(&mut self) -> Option<Expression> {
        if self.at(TokenKind::Minus) {
            let operator = self.advance();
            let operand = self.with_syntax_nesting(operator.span, |parser| parser.parse_unary())?;
            let span = self.cover(operator.span, operand.span());
            return Some(Expression::Unary(UnaryExpr {
                operator: UnaryOperator::Negate,
                operator_span: operator.span,
                operand: Box::new(operand),
                span,
            }));
        }

        if self.starts_object_cast() {
            return self.parse_object_cast();
        }

        self.parse_postfix()
    }

    fn starts_object_cast(&self) -> bool {
        if !self.at(TokenKind::LeftParen) {
            return false;
        }
        let (right_paren_distance, valid_target) = if self.peek_ahead(1).kind
            == TokenKind::Identifier
            && self.lexeme(self.peek_ahead(1)) == "shared"
        {
            (
                3,
                self.peek_ahead(2).kind == TokenKind::Identifier
                    && self.peek_ahead(3).kind == TokenKind::RightParen,
            )
        } else {
            (
                2,
                self.peek_ahead(1).kind == TokenKind::Identifier
                    && self.peek_ahead(2).kind == TokenKind::RightParen,
            )
        };
        valid_target && self.starts_cast_operand(right_paren_distance + 1)
    }

    fn starts_cast_operand(&self, distance: usize) -> bool {
        if self.peek_ahead(distance).kind == TokenKind::LeftParen
            && self.peek_ahead(distance + 1).kind == TokenKind::RightParen
        {
            return false;
        }
        matches!(
            self.peek_ahead(distance).kind,
            TokenKind::Identifier
                | TokenKind::SelfValue
                | TokenKind::NumericLiteral(_)
                | TokenKind::True
                | TokenKind::False
                | TokenKind::LeftParen
        )
    }

    fn parse_object_cast(&mut self) -> Option<Expression> {
        let left_paren = self.advance();
        let target_mode = if self.at_contextual("shared") {
            ObjectCastTargetMode::Shared {
                shared_span: self.advance().span,
            }
        } else {
            ObjectCastTargetMode::Plain
        };
        let target = self.parse_name("expected a cast target")?;
        let _right_paren = self.expect(TokenKind::RightParen, "`)` after the cast target")?;
        let source = self.with_syntax_nesting(left_paren.span, |parser| parser.parse_unary())?;
        let span = self.cover(left_paren.span, source.span());
        Some(Expression::ObjectCast(ObjectCastExpr {
            target,
            target_mode,
            source: Box::new(source),
            span,
        }))
    }

    fn parse_postfix(&mut self) -> Option<Expression> {
        let mut expression = self.parse_primary()?;

        while self.at_any(&[TokenKind::LeftParen, TokenKind::Dot]) {
            if self.at(TokenKind::LeftParen) {
                let left_paren_span = self.peek().span;
                expression = self.with_syntax_nesting(left_paren_span, move |parser| {
                    parser.finish_call(expression)
                })?;
            } else {
                expression = self.finish_member_access(expression)?;
            }
        }

        Some(expression)
    }

    fn finish_member_access(&mut self, receiver: Expression) -> Option<Expression> {
        let dot = self.advance();
        let member = self.parse_name("expected a member name after `.`")?;
        let span = self.cover(receiver.span(), member.span);
        Some(Expression::MemberAccess(MemberAccessExpr {
            receiver: Box::new(receiver),
            dot_span: dot.span,
            member,
            span,
        }))
    }

    fn finish_call(&mut self, callee: Expression) -> Option<Expression> {
        let (arguments, end_span) = self.parse_call_arguments()?;
        Some(Expression::Call(CallExpr {
            span: self.cover(callee.span(), end_span),
            callee: Box::new(callee),
            arguments,
        }))
    }

    pub(super) fn parse_call_arguments(&mut self) -> Option<(Vec<Expression>, Span)> {
        let left_paren = self.advance();
        debug_assert_eq!(left_paren.kind, TokenKind::LeftParen);
        let mut arguments = Vec::new();
        let mut valid = true;

        if self.consume(TokenKind::RightParen).is_some() {
            let right_paren = self.previous();
            return Some((arguments, right_paren.span));
        }

        loop {
            if self.at_any(&[
                TokenKind::RightParen,
                TokenKind::Semicolon,
                TokenKind::RightBrace,
                TokenKind::Eof,
            ]) {
                self.report(
                    EXPECTED_EXPRESSION,
                    "expected a call argument",
                    self.peek().span,
                    "expected an expression here",
                );
                valid = false;
                break;
            }

            match self.parse_expression() {
                Some(argument) => arguments.push(argument),
                None => {
                    valid = false;
                    self.synchronize_argument();
                }
            }

            if self.consume(TokenKind::Comma).is_some() {
                if self.at(TokenKind::RightParen) {
                    self.report(
                        EXPECTED_EXPRESSION,
                        "expected a call argument after `,`",
                        self.peek().span,
                        "trailing commas are not supported",
                    );
                    valid = false;
                    break;
                }
                continue;
            }

            if self.at(TokenKind::RightParen) {
                break;
            }

            if self.starts_expression() {
                self.report(
                    EXPECTED_TOKEN,
                    "expected `,` between call arguments",
                    self.peek().span,
                    "insert `,` before this argument",
                );
                continue;
            }

            break;
        }

        let right_paren = self.expect(TokenKind::RightParen, "`)` after call arguments");
        let end_span = right_paren
            .map(|token| token.span)
            .or_else(|| arguments.last().map(Expression::span))
            .unwrap_or(left_paren.span);
        if !valid {
            return None;
        }

        Some((arguments, end_span))
    }

    fn parse_primary(&mut self) -> Option<Expression> {
        if let Some(token) = self.consume(TokenKind::Identifier) {
            let name = Name {
                text: self.lexeme(token).to_owned(),
                span: token.span,
            };
            return Some(Expression::Identifier(IdentifierExpr {
                span: token.span,
                name,
            }));
        }

        if let Some(token) = self.consume_numeric_literal() {
            let TokenKind::NumericLiteral(kind) = token.kind else {
                unreachable!("numeric consumer returned a non-numeric token")
            };
            return Some(Expression::NumericLiteral(NumericLiteralExpr {
                kind,
                spelling: self.lexeme(token).to_owned(),
                span: token.span,
            }));
        }

        if let Some(token) = self.consume(TokenKind::True) {
            return Some(Expression::Boolean(BooleanExpr {
                value: true,
                span: token.span,
            }));
        }

        if let Some(token) = self.consume(TokenKind::False) {
            return Some(Expression::Boolean(BooleanExpr {
                value: false,
                span: token.span,
            }));
        }

        if let Some(token) = self.consume(TokenKind::SelfValue) {
            return Some(Expression::SelfValue(SelfExpr { span: token.span }));
        }

        if let Some(left_paren) = self.consume(TokenKind::LeftParen) {
            let expression =
                self.with_syntax_nesting(left_paren.span, |parser| parser.parse_expression())?;
            let right_paren = self.expect(TokenKind::RightParen, "`)` after the expression");
            let end_span = right_paren
                .map(|token| token.span)
                .unwrap_or_else(|| expression.span());
            return Some(Expression::Grouped(GroupedExpr {
                span: self.cover(left_paren.span, end_span),
                expression: Box::new(expression),
            }));
        }

        if self.at(TokenKind::Invalid) {
            self.advance();
            return None;
        }

        self.report(
            EXPECTED_EXPRESSION,
            "expected an expression",
            self.peek().span,
            "expected an identifier, literal, `self`, unary `-`, or `(`",
        );
        None
    }
}
