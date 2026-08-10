//! Expression grammar, precedence, grouping, and calls.

use super::*;

impl Parser<'_> {
    pub(super) fn parse_expression(&mut self) -> Option<Expression> {
        let outermost = self.expression_parse_depth == 0;
        self.expression_parse_depth += 1;
        let expression = self
            .parse_shift()
            .and_then(|source| self.parse_expression_tail(source));
        self.expression_parse_depth -= 1;

        if outermost
            && expression
                .as_ref()
                .is_some_and(super::logical_depth::exceeds_limit)
        {
            let span = expression
                .as_ref()
                .expect("depth check requires an expression")
                .span();
            self.report_excessive_logical_depth(span);
            self.recover_from_excessive_nesting();
            return None;
        }
        expression
    }

    fn parse_expression_tail(&mut self, source: Expression) -> Option<Expression> {
        let source = self.parse_bitwise_tail(source)?;
        let first = self.parse_expression_suffix(source)?;
        if !self.at_any(&[TokenKind::AndAnd, TokenKind::OrOr]) {
            return Some(first);
        }
        self.parse_logical_tail(first)
    }

    fn parse_logical_tail(&mut self, first: Expression) -> Option<Expression> {
        let mut operands = vec![first];
        let mut operators = Vec::new();

        while self.at_any(&[TokenKind::AndAnd, TokenKind::OrOr]) {
            let token = self.advance();
            let (operator, spelling) = match token.kind {
                TokenKind::AndAnd => (LogicalOperator::And, "&&"),
                TokenKind::OrOr => (LogicalOperator::Or, "||"),
                _ => unreachable!("logical parser accepted a non-logical token"),
            };
            if !self.starts_expression() {
                self.report(
                    EXPECTED_EXPRESSION,
                    format!("expected a right operand after `{spelling}`"),
                    self.peek().span,
                    "expected an expression here",
                );
                return None;
            }

            let source = self.parse_shift()?;
            let source = self.parse_bitwise_tail(source)?;
            let right = self.parse_expression_suffix(source)?;
            while operators.last().is_some_and(|(pending, _)| {
                logical_precedence(*pending) >= logical_precedence(operator)
            }) {
                self.reduce_logical_expression(&mut operands, operators.pop().unwrap());
            }
            operators.push((operator, token.span));
            operands.push(right);
        }

        while let Some(operator) = operators.pop() {
            self.reduce_logical_expression(&mut operands, operator);
        }
        operands.pop()
    }

    fn reduce_logical_expression(
        &self,
        operands: &mut Vec<Expression>,
        (operator, operator_span): (LogicalOperator, Span),
    ) {
        let right = operands
            .pop()
            .expect("logical operator must have a right operand");
        let left = operands
            .pop()
            .expect("logical operator must have a left operand");
        let span = self.cover(left.span(), right.span());
        operands.push(Expression::Logical(LogicalExpr {
            left: Box::new(left),
            operator,
            operator_span,
            right: Box::new(right),
            span,
        }));
    }

    fn parse_expression_suffix(&mut self, mut source: Expression) -> Option<Expression> {
        if comparison_operator(self.peek().kind).is_some() {
            source = self.parse_comparison_suffix(source)?;
        }
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

    fn parse_comparison_suffix(&mut self, left: Expression) -> Option<Expression> {
        let operator = comparison_operator(self.peek().kind)
            .expect("comparison suffix must start at a comparison operator");
        let operator_token = self.advance();
        let right = self.parse_shift()?;
        let right = self.parse_bitwise_tail(right)?;
        let span = self.cover(left.span(), right.span());
        let expression = Expression::Binary(BinaryExpr {
            left: Box::new(left),
            operator,
            operator_span: operator_token.span,
            right: Box::new(right),
            span,
        });

        let Some(_) = comparison_operator(self.peek().kind) else {
            return Some(expression);
        };
        let chained = self.advance();
        self.report(
            INVALID_COMPARISON,
            "comparison operators cannot be chained",
            chained.span,
            "group separate comparisons explicitly",
        );

        // Consume the rest of this chain so statement-level recovery resumes
        // after the complete invalid expression rather than at each operator.
        let _ = self
            .parse_shift()
            .and_then(|first| self.parse_bitwise_tail(first));
        while comparison_operator(self.peek().kind).is_some() {
            self.advance();
            let _ = self
                .parse_shift()
                .and_then(|first| self.parse_bitwise_tail(first));
        }
        None
    }

    fn parse_bitwise_tail(&mut self, first: Expression) -> Option<Expression> {
        let mut operands = vec![first];
        let mut operators = Vec::new();

        while let Some(operator) = bitwise_operator(self.peek().kind) {
            let token = self.advance();
            let right = self.parse_shift()?;
            while operators.last().is_some_and(|(pending, _)| {
                bitwise_precedence(*pending) >= bitwise_precedence(operator)
            }) {
                self.reduce_bitwise_expression(&mut operands, operators.pop().unwrap());
            }
            operators.push((operator, token.span));
            operands.push(right);
        }

        while let Some(operator) = operators.pop() {
            self.reduce_bitwise_expression(&mut operands, operator);
        }
        operands.pop()
    }

    fn reduce_bitwise_expression(
        &self,
        operands: &mut Vec<Expression>,
        (operator, operator_span): (BinaryOperator, Span),
    ) {
        let right = operands
            .pop()
            .expect("bitwise operator must have a right operand");
        let left = operands
            .pop()
            .expect("bitwise operator must have a left operand");
        let span = self.cover(left.span(), right.span());
        operands.push(Expression::Binary(BinaryExpr {
            left: Box::new(left),
            operator,
            operator_span,
            right: Box::new(right),
            span,
        }));
    }

    fn parse_shift(&mut self) -> Option<Expression> {
        // Parse the first additive operand here instead of entering another
        // wrapper frame. Deep primary-expression recursion shares a fixed
        // syntax budget, so adding a precedence tier must not reduce the
        // amount of source nesting that budget can safely accept.
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

        while self.at_any(&[TokenKind::ShiftLeft, TokenKind::ShiftRight]) {
            let operator = self.advance();
            let right = self.parse_additive()?;
            let kind = match operator.kind {
                TokenKind::ShiftLeft => BinaryOperator::ShiftLeft,
                TokenKind::ShiftRight => BinaryOperator::ShiftRight,
                _ => unreachable!("shift parser accepted a non-shift operator"),
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

        while self.at_any(&[TokenKind::Star, TokenKind::Slash, TokenKind::Percent]) {
            let operator = self.advance();
            let right = self.parse_unary()?;
            let kind = match operator.kind {
                TokenKind::Star => BinaryOperator::Multiply,
                TokenKind::Slash => BinaryOperator::Divide,
                TokenKind::Percent => BinaryOperator::Remainder,
                _ => unreachable!("multiplicative parser accepted another operator"),
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

    fn parse_unary(&mut self) -> Option<Expression> {
        let mut prefixes = Vec::new();
        while self.at_any(&[
            TokenKind::Minus,
            TokenKind::Bang,
            TokenKind::Tilde,
            TokenKind::Star,
        ]) {
            if self.nesting_depth + prefixes.len() >= MAX_SYNTAX_NESTING {
                self.report_excessive_nesting(self.peek().span);
                self.recover_from_excessive_nesting();
                return None;
            }
            let token = self.advance();
            prefixes.push((
                match token.kind {
                    TokenKind::Minus => UnaryOperator::Negate,
                    TokenKind::Bang => UnaryOperator::LogicalNot,
                    TokenKind::Tilde => UnaryOperator::BitwiseComplement,
                    TokenKind::Star => UnaryOperator::Dereference,
                    _ => unreachable!("unary parser accepted a non-unary operator"),
                },
                token.span,
            ));
        }

        self.nesting_depth += prefixes.len();
        let operand = if self.starts_primitive_cast() {
            self.parse_primitive_cast()
        } else if self.starts_object_cast() {
            self.parse_object_cast()
        } else {
            self.parse_postfix()
        };
        self.nesting_depth -= prefixes.len();

        let mut expression = operand?;
        for (operator, operator_span) in prefixes.into_iter().rev() {
            let span = self.cover(operator_span, expression.span());
            expression = Expression::Unary(UnaryExpr {
                operator,
                operator_span,
                operand: Box::new(expression),
                span,
            });
        }
        Some(expression)
    }

    fn starts_primitive_cast(&self) -> bool {
        self.at(TokenKind::LeftParen)
            && matches!(
                self.peek_ahead(1).kind,
                TokenKind::I64 | TokenKind::U64 | TokenKind::U8 | TokenKind::F64 | TokenKind::Bool
            )
            && self.peek_ahead(2).kind == TokenKind::RightParen
            && self.starts_cast_operand(3, true)
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
            && self.starts_cast_operand(right_paren_distance + 1, false)
    }

    fn starts_cast_operand(&self, distance: usize, allow_primitive_prefix: bool) -> bool {
        if self.peek_ahead(distance).kind == TokenKind::LeftParen
            && self.peek_ahead(distance + 1).kind == TokenKind::RightParen
        {
            return false;
        }
        (allow_primitive_prefix
            && matches!(
                self.peek_ahead(distance).kind,
                TokenKind::Minus | TokenKind::Bang | TokenKind::Tilde
            ))
            || (matches!(
                self.peek_ahead(distance).kind,
                TokenKind::I64 | TokenKind::U64 | TokenKind::U8 | TokenKind::F64 | TokenKind::Bool
            ) && self.peek_ahead(distance + 1).kind == TokenKind::DoubleColon)
            || matches!(
                self.peek_ahead(distance).kind,
                TokenKind::Identifier
                    | TokenKind::SelfValue
                    | TokenKind::NumericLiteral(_)
                    | TokenKind::ByteLiteral
                    | TokenKind::StringLiteral
                    | TokenKind::True
                    | TokenKind::False
                    | TokenKind::None
                    | TokenKind::Star
                    | TokenKind::LeftParen
            )
    }

    fn parse_primitive_cast(&mut self) -> Option<Expression> {
        let left_paren = self.advance();
        let target = self.advance();
        let target_kind = match target.kind {
            TokenKind::I64 => PrimitiveType::I64,
            TokenKind::U64 => PrimitiveType::U64,
            TokenKind::U8 => PrimitiveType::U8,
            TokenKind::F64 => PrimitiveType::F64,
            TokenKind::Bool => PrimitiveType::Bool,
            _ => unreachable!("primitive-cast parser accepted a non-primitive target"),
        };
        let _right_paren = self.expect(TokenKind::RightParen, "`)` after the cast target")?;
        let source = self.with_syntax_nesting(left_paren.span, |parser| parser.parse_unary())?;
        let span = self.cover(left_paren.span, source.span());
        Some(Expression::PrimitiveCast(PrimitiveCastExpr {
            target: target_kind,
            target_span: target.span,
            source: Box::new(source),
            span,
        }))
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
                    if self.recovering_from_excessive_nesting {
                        return None;
                    }
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

        if self.at_contextual("some")
            && self.peek_ahead(1).kind == TokenKind::LeftParen
            && self.peek_ahead(2).kind != TokenKind::RightParen
        {
            return self.parse_present();
        }

        if self.at(TokenKind::Identifier)
            || (self.at_primitive_type_name() && self.peek_ahead(1).kind == TokenKind::DoubleColon)
        {
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

        if self.at(TokenKind::ByteLiteral) {
            return Some(self.parse_byte_literal());
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
            "expected an identifier, literal, `none`, `self`, prefix operator, or `(`",
        );
        None
    }

    // Keep contextual optional construction out of the recursively entered
    // primary-expression frame. The parser's common nesting limit is chosen
    // to fail predictably even on the small stacks used by test runners.
    fn parse_present(&mut self) -> Option<Expression> {
        let some = self.advance();
        let left_paren = self.advance();
        let value =
            self.with_syntax_nesting(left_paren.span, |parser| parser.parse_expression())?;
        let right_paren = self.expect(TokenKind::RightParen, "`)` after `some` payload");
        let end = right_paren
            .map(|token| token.span)
            .unwrap_or_else(|| value.span());
        Some(Expression::Present(PresentExpr {
            some_span: some.span,
            value: Box::new(value),
            span: self.cover(some.span, end),
        }))
    }

    fn parse_byte_literal(&mut self) -> Expression {
        let token = self.advance();
        debug_assert_eq!(token.kind, TokenKind::ByteLiteral);
        Expression::ByteLiteral(ByteLiteralExpr {
            value: decode_byte_literal(self.lexeme(token)),
            span: token.span,
        })
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
        let mut distance = start;
        loop {
            let kind = self.peek_ahead(distance).kind;
            let followed_by_separator =
                self.peek_ahead(distance + 1).kind == TokenKind::DoubleColon;
            if kind != TokenKind::Identifier
                && !(matches!(
                    kind,
                    TokenKind::I64
                        | TokenKind::U64
                        | TokenKind::U8
                        | TokenKind::F64
                        | TokenKind::Bool
                ) && followed_by_separator)
            {
                return None;
            }
            distance += 1;
            if !followed_by_separator {
                return Some(distance);
            }
            distance += 1;
        }
    }
}

const fn comparison_operator(kind: TokenKind) -> Option<BinaryOperator> {
    match kind {
        TokenKind::EqualEqual => Some(BinaryOperator::Equal),
        TokenKind::BangEqual => Some(BinaryOperator::NotEqual),
        TokenKind::Less => Some(BinaryOperator::LessThan),
        TokenKind::LessEqual => Some(BinaryOperator::LessEqual),
        TokenKind::Greater => Some(BinaryOperator::GreaterThan),
        TokenKind::GreaterEqual => Some(BinaryOperator::GreaterEqual),
        _ => None,
    }
}

const fn logical_precedence(operator: LogicalOperator) -> u8 {
    match operator {
        LogicalOperator::And => 2,
        LogicalOperator::Or => 1,
    }
}

const fn bitwise_operator(kind: TokenKind) -> Option<BinaryOperator> {
    match kind {
        TokenKind::Ampersand => Some(BinaryOperator::BitwiseAnd),
        TokenKind::Caret => Some(BinaryOperator::BitwiseXor),
        TokenKind::Pipe => Some(BinaryOperator::BitwiseOr),
        _ => None,
    }
}

fn bitwise_precedence(operator: BinaryOperator) -> u8 {
    match operator {
        BinaryOperator::BitwiseAnd => 3,
        BinaryOperator::BitwiseXor => 2,
        BinaryOperator::BitwiseOr => 1,
        _ => unreachable!("bitwise parser accepted a non-bitwise operator"),
    }
}
