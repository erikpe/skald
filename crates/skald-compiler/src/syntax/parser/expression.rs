//! Expression grammar, precedence, grouping, and calls.

use super::*;

impl Parser<'_> {
    pub(super) fn parse_expression(&mut self) -> Option<Expression> {
        self.parse_additive()
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
            let operand = self.parse_unary()?;
            let span = self.cover(operator.span, operand.span());
            return Some(Expression::Unary(UnaryExpr {
                operator: UnaryOperator::Negate,
                operator_span: operator.span,
                operand: Box::new(operand),
                span,
            }));
        }

        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> Option<Expression> {
        let mut expression = self.parse_primary()?;

        while self.at(TokenKind::LeftParen) {
            expression = self.finish_call(expression)?;
        }

        Some(expression)
    }

    fn finish_call(&mut self, callee: Expression) -> Option<Expression> {
        let _left_paren = self.advance();
        let mut arguments = Vec::new();
        let mut valid = true;

        if self.consume(TokenKind::RightParen).is_some() {
            let right_paren = self.previous();
            let span = self.cover(callee.span(), right_paren.span);
            return Some(Expression::Call(CallExpr {
                callee: Box::new(callee),
                arguments,
                span,
            }));
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
            .unwrap_or_else(|| callee.span());
        if !valid {
            return None;
        }

        Some(Expression::Call(CallExpr {
            span: self.cover(callee.span(), end_span),
            callee: Box::new(callee),
            arguments,
        }))
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

        if let Some(left_paren) = self.consume(TokenKind::LeftParen) {
            let expression = self.parse_expression();
            let right_paren = self.expect(TokenKind::RightParen, "`)` after the expression");
            let expression = expression?;
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
            "expected an identifier, literal, unary `-`, or `(`",
        );
        None
    }
}
