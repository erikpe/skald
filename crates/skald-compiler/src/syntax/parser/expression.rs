//! Expression grammar, precedence, grouping, and calls.

use super::*;

impl Parser<'_> {
    pub(super) fn parse_expression(&mut self) -> Option<Expression> {
        let source = self.parse_additive()?;
        if !self.at_contextual("is") {
            return Some(source);
        }

        let is_token = self.advance();
        if self.at_contextual("some") || self.at(TokenKind::None) {
            let target = self.advance();
            let kind = if target.kind == TokenKind::None {
                PresenceTestKind::None
            } else {
                PresenceTestKind::Some
            };
            let span = self.cover(source.span(), target.span);
            let expression = Expression::PresenceTest(PresenceTestExpr {
                source: Box::new(source),
                is_span: is_token.span,
                kind,
                target_span: target.span,
                span,
            });
            return self.finish_type_or_presence_test(expression);
        }
        let target = self.parse_name_path("expected a class, interface, or `Obj` after `is`")?;
        let span = self.cover(source.span(), target.span);
        let expression = Expression::TypeTest(TypeTestExpr {
            source: Box::new(source),
            is_span: is_token.span,
            target,
            span,
        });
        self.finish_type_or_presence_test(expression)
    }

    fn finish_type_or_presence_test(&mut self, expression: Expression) -> Option<Expression> {
        if self.at_contextual("is") {
            let chained = self.advance();
            self.report(
                INVALID_TYPE_TEST,
                "type tests cannot be chained",
                chained.span,
                "group separate tests explicitly",
            );
            if self.at_contextual("some") || self.at(TokenKind::None) {
                self.advance();
            } else {
                let _ = self.parse_name_path("expected a type or presence state after `is`");
            }
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
        if self.at_any(&[TokenKind::Minus, TokenKind::Star]) {
            let operator = self.advance();
            let operand = self.with_syntax_nesting(operator.span, |parser| parser.parse_unary())?;
            let span = self.cover(operator.span, operand.span());
            return Some(Expression::Unary(UnaryExpr {
                operator: match operator.kind {
                    TokenKind::Minus => UnaryOperator::Negate,
                    TokenKind::Star => UnaryOperator::Dereference,
                    _ => unreachable!("unary parser accepted a non-unary operator"),
                },
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
        let target_start = if self.peek_ahead(1).kind == TokenKind::Identifier
            && self.lexeme(self.peek_ahead(1)) == "shared"
        {
            2
        } else {
            1
        };
        let Some(right_paren_distance) = self.name_path_end(target_start) else {
            return false;
        };
        self.peek_ahead(right_paren_distance).kind == TokenKind::RightParen
            && self.starts_cast_operand(right_paren_distance + 1)
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
                | TokenKind::StringLiteral
                | TokenKind::True
                | TokenKind::False
                | TokenKind::None
                | TokenKind::Star
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
        let target = self.parse_name_path("expected a cast target")?;
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
        let mut postfix_depth = 0usize;

        while self.at_any(&[
            TokenKind::LeftParen,
            TokenKind::LeftBracket,
            TokenKind::Dot,
            TokenKind::Arrow,
            TokenKind::Bang,
        ]) {
            if self.nesting_depth + postfix_depth >= MAX_SYNTAX_NESTING {
                self.report_excessive_nesting(self.peek().span);
                self.recover_from_excessive_nesting();
                return None;
            }
            postfix_depth += 1;
            if self.at(TokenKind::LeftParen) {
                let left_paren_span = self.peek().span;
                expression = self.with_syntax_nesting(left_paren_span, move |parser| {
                    parser.finish_call(expression)
                })?;
            } else if self.at(TokenKind::Bang) {
                let bang = self.advance();
                let span = self.cover(expression.span(), bang.span);
                expression = Expression::Unwrap(UnwrapExpr {
                    source: Box::new(expression),
                    bang_span: bang.span,
                    span,
                });
            } else if self.at(TokenKind::LeftBracket)
                || (self.at(TokenKind::Arrow) && self.peek_ahead(1).kind == TokenKind::LeftBracket)
            {
                let start_span = self.peek().span;
                expression = self.with_syntax_nesting(start_span, move |parser| {
                    parser.finish_array_projection(expression)
                })?;
            } else {
                expression = self.finish_member_access(expression)?;
            }
        }

        Some(expression)
    }

    fn finish_member_access(&mut self, receiver: Expression) -> Option<Expression> {
        let operator = self.advance();
        let member = self.parse_name(match operator.kind {
            TokenKind::Dot => "expected a member name after `.`",
            TokenKind::Arrow => "expected a member name after `->`",
            _ => unreachable!("member parser accepted a non-member operator"),
        })?;
        let span = self.cover(receiver.span(), member.span);
        Some(Expression::MemberAccess(MemberAccessExpr {
            receiver: Box::new(receiver),
            operator: match operator.kind {
                TokenKind::Dot => MemberAccessOperator::Dot {
                    span: operator.span,
                },
                TokenKind::Arrow => MemberAccessOperator::Arrow {
                    span: operator.span,
                },
                _ => unreachable!("member parser accepted a non-member operator"),
            },
            member,
            span,
        }))
    }

    fn finish_call(&mut self, callee: Expression) -> Option<Expression> {
        let (arguments, end_span) = self.parse_construction_arguments()?;
        Some(Expression::Call(CallExpr {
            span: self.cover(callee.span(), end_span),
            callee: Box::new(callee),
            arguments,
        }))
    }

    fn parse_construction_arguments(&mut self) -> Option<(CallArguments, Span)> {
        if self.at_contextual_copy_arguments() {
            self.parse_copy_call_arguments()
        } else {
            self.parse_call_arguments()
                .map(|(arguments, span)| (CallArguments::Ordinary(arguments), span))
        }
    }

    fn at_contextual_copy_arguments(&self) -> bool {
        self.peek().kind == TokenKind::LeftParen
            && self.peek_ahead(1).kind == TokenKind::Identifier
            && self.lexeme(self.peek_ahead(1)) == "copy"
            && self.starts_expression_ahead(2)
    }

    fn parse_copy_call_arguments(&mut self) -> Option<(CallArguments, Span)> {
        let left_paren = self.advance();
        debug_assert_eq!(left_paren.kind, TokenKind::LeftParen);
        let copy_token = self.advance();
        debug_assert_eq!(self.lexeme(copy_token), "copy");
        let source = self.parse_expression()?;
        let right_paren = self.expect(
            TokenKind::RightParen,
            "`)` after the explicit copy-construction source",
        )?;
        Some((
            CallArguments::Copy {
                copy_span: copy_token.span,
                source: Box::new(source),
            },
            right_paren.span,
        ))
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
        if self.starts_array_construction(false) {
            return self.parse_array_construction(false);
        }
        if self.at_contextual("new") && self.starts_array_construction(true) {
            return self.parse_array_construction(true);
        }
        if self.at_contextual("new") && self.name_path_followed_by(1, TokenKind::LeftParen) {
            return self.parse_allocation();
        }

        if let Some(token) = self.consume(TokenKind::None) {
            return Some(Expression::Absent(AbsentExpr { span: token.span }));
        }

        if self.at(TokenKind::Identifier) {
            let name = self.parse_name_path("expected a declaration or binding name")?;
            return Some(Expression::Identifier(IdentifierExpr {
                span: name.span,
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

        if let Some(token) = self.consume(TokenKind::StringLiteral) {
            return Some(Expression::StringLiteral(StringLiteralExpr {
                bytes: decode_string_literal(self.lexeme(token)),
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
            "expected an identifier, literal, `none`, `self`, unary `-`, or `(`",
        );
        None
    }

    fn parse_allocation(&mut self) -> Option<Expression> {
        let new_token = self.advance();
        debug_assert_eq!(self.lexeme(new_token), "new");
        let target = self.parse_name_path("expected a concrete class after `new`")?;
        let left_paren = self.peek().span;
        let (arguments, end_span) =
            self.with_syntax_nesting(left_paren, |parser| parser.parse_construction_arguments())?;
        Some(Expression::Allocation(Box::new(AllocationExpr {
            new_span: new_token.span,
            span: self.cover(new_token.span, end_span),
            target,
            arguments,
        })))
    }

    fn name_path_followed_by(&self, start: usize, follower: TokenKind) -> bool {
        self.name_path_end(start)
            .is_some_and(|end| self.peek_ahead(end).kind == follower)
    }

    fn name_path_end(&self, start: usize) -> Option<usize> {
        (self.peek_ahead(start).kind == TokenKind::Identifier).then_some(())?;
        let mut distance = start + 1;
        while self.peek_ahead(distance).kind == TokenKind::DoubleColon {
            (self.peek_ahead(distance + 1).kind == TokenKind::Identifier).then_some(())?;
            distance += 2;
        }
        Some(distance)
    }
}
