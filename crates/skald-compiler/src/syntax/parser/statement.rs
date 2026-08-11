//! Blocks, statements, conditionals, locals, and returns.

use super::{declaration::TypeContext, *};

impl Parser<'_> {
    pub(super) fn parse_block(&mut self) -> Option<Block> {
        self.parse_block_with_recovery(false)
    }

    fn parse_conditional_arm_block(&mut self) -> Option<Block> {
        self.parse_block_with_recovery(true)
    }

    fn parse_block_with_recovery(
        &mut self,
        stop_at_conditional_continuation: bool,
    ) -> Option<Block> {
        let left_brace = self.expect(TokenKind::LeftBrace, "`{` to start a block")?;
        self.brace_depth += 1;
        let block = self.with_syntax_nesting(left_brace.span, move |parser| {
            parser.parse_block_contents(left_brace, stop_at_conditional_continuation)
        });
        self.brace_depth -= 1;
        block
    }

    fn parse_block_contents(
        &mut self,
        left_brace: Token,
        stop_at_conditional_continuation: bool,
    ) -> Option<Block> {
        let mut statements = Vec::new();

        while !self.at_any(&[TokenKind::RightBrace, TokenKind::Eof]) {
            if stop_at_conditional_continuation && self.at_any(&[TokenKind::Elif, TokenKind::Else])
            {
                break;
            }
            if self.at_any(&[TokenKind::Fn, TokenKind::Extern]) {
                // A top-level declaration is a strong indication that the
                // preceding block is missing its closing brace.
                break;
            }
            if self.at(TokenKind::Invalid) {
                self.advance();
                continue;
            }

            let before = self.current;
            if let Some(statement) = self.parse_statement() {
                statements.push(statement);
            } else {
                self.synchronize_statement();
            }
            if self.current == before {
                self.advance();
            }
        }

        let right_brace = self.expect(TokenKind::RightBrace, "`}` after the block");
        let end_span = right_brace
            .map(|token| token.span)
            .or_else(|| statements.last().map(Statement::span))
            .unwrap_or(left_brace.span);

        Some(Block {
            statements,
            span: self.cover(left_brace.span, end_span),
        })
    }

    fn parse_statement(&mut self) -> Option<Statement> {
        if self.at_contextual("super") && self.peek_ahead(1).kind == TokenKind::LeftParen {
            return self
                .parse_base_initialization()
                .map(Statement::BaseInitialization);
        }
        if self.at(TokenKind::Var) {
            return self.parse_local().map(Statement::Local);
        }
        if self.at(TokenKind::Return) {
            return self.parse_return().map(Statement::Return);
        }
        if self.at(TokenKind::Break) {
            return Some(Statement::Break(self.parse_break()));
        }
        if self.at(TokenKind::Continue) {
            return Some(Statement::Continue(self.parse_continue()));
        }
        if self.at(TokenKind::If) {
            return self.parse_conditional().map(Statement::Conditional);
        }
        if self.at(TokenKind::While) {
            return self.parse_while().map(Statement::While);
        }
        if self.at_any(&[TokenKind::Elif, TokenKind::Else]) {
            self.parse_misplaced_conditional_continuation();
            return None;
        }
        if self.at(TokenKind::LeftBrace) {
            return self.parse_block().map(Statement::Block);
        }
        if self.at_any(&[TokenKind::Mut, TokenKind::Ref]) {
            self.report(
                EXPECTED_STATEMENT,
                "local alias bindings are not supported",
                self.peek().span,
                "`ref` and `mut ref` are supported only on parameters",
            );
            self.discard_misplaced_alias_binding();
            return None;
        }

        if self.starts_expression() {
            return self.parse_expression_or_assignment();
        }

        self.report(
            EXPECTED_STATEMENT,
            "expected a statement",
            self.peek().span,
            "expected `var`, `return`, `if`, `while`, `break`, `continue`, an expression, a field assignment, or a nested block",
        );
        None
    }

    fn parse_while(&mut self) -> Option<WhileStatement> {
        let while_token = self.advance();
        self.expect(TokenKind::LeftParen, "`(` after `while`");
        let condition = if self.at_any(&[
            TokenKind::RightParen,
            TokenKind::LeftBrace,
            TokenKind::RightBrace,
            TokenKind::Eof,
        ]) {
            self.report(
                EXPECTED_EXPRESSION,
                "expected a condition after `while (`",
                self.peek().span,
                "expected a boolean expression here",
            );
            None
        } else {
            self.parse_expression()
        };
        self.expect(TokenKind::RightParen, "`)` after the `while` condition");
        let body = self.parse_block();

        match (condition, body) {
            (Some(condition), Some(body)) => Some(WhileStatement {
                while_span: while_token.span,
                span: self.cover(while_token.span, body.span),
                condition,
                body,
            }),
            _ => None,
        }
    }

    fn parse_base_initialization(&mut self) -> Option<BaseInitializationStatement> {
        let super_token = self.advance();
        let (arguments, arguments_end) = self.parse_call_arguments()?;
        let semicolon = self.expect(
            TokenKind::Semicolon,
            "`;` after the base-initialization statement",
        );
        let end_span = semicolon.map_or(arguments_end, |token| token.span);
        Some(BaseInitializationStatement {
            super_span: super_token.span,
            arguments,
            span: self.cover(super_token.span, end_span),
        })
    }

    fn discard_misplaced_alias_binding(&mut self) {
        while !self.at_any(&[
            TokenKind::Semicolon,
            TokenKind::RightBrace,
            TokenKind::Fn,
            TokenKind::Extern,
            TokenKind::Class,
            TokenKind::Eof,
        ]) {
            self.advance();
        }
        self.consume(TokenKind::Semicolon);
    }

    fn parse_conditional(&mut self) -> Option<ConditionalStatement> {
        let if_token = self.advance();
        let if_arm = self.parse_conditional_arm(if_token, "if");
        let mut elif_arms = Vec::new();
        let mut valid = if_arm.is_some();

        while let Some(elif_token) = self.consume(TokenKind::Elif) {
            match self.parse_conditional_arm(elif_token, "elif") {
                Some(arm) => elif_arms.push(arm),
                None => valid = false,
            }
        }

        let else_block = if self.consume(TokenKind::Else).is_some() {
            let block = self.parse_block();
            if block.is_none() {
                valid = false;
            }
            block
        } else {
            None
        };

        let if_arm = if_arm?;
        let end_span = else_block
            .as_ref()
            .map(|block| block.span)
            .or_else(|| elif_arms.last().map(|arm| arm.span))
            .unwrap_or(if_arm.span);
        valid.then_some(ConditionalStatement {
            if_arm,
            elif_arms,
            else_block,
            span: self.cover(if_token.span, end_span),
        })
    }

    fn parse_conditional_arm(
        &mut self,
        keyword: Token,
        keyword_name: &'static str,
    ) -> Option<ConditionalArm> {
        self.expect(
            TokenKind::LeftParen,
            if keyword_name == "if" {
                "`(` after `if`"
            } else {
                "`(` after `elif`"
            },
        );
        let condition = if self.at_any(&[
            TokenKind::RightParen,
            TokenKind::LeftBrace,
            TokenKind::Elif,
            TokenKind::Else,
            TokenKind::RightBrace,
            TokenKind::Eof,
        ]) {
            self.report(
                EXPECTED_EXPRESSION,
                format!("expected a condition after `{keyword_name} (`"),
                self.peek().span,
                "expected a boolean expression here",
            );
            None
        } else {
            self.parse_expression()
        };
        self.expect(
            TokenKind::RightParen,
            if keyword_name == "if" {
                "`)` after the `if` condition"
            } else {
                "`)` after the `elif` condition"
            },
        );
        let body = self.parse_conditional_arm_block();

        match (condition, body) {
            (Some(condition), Some(body)) => Some(ConditionalArm {
                span: self.cover(keyword.span, body.span),
                condition,
                body,
            }),
            _ => None,
        }
    }

    fn parse_misplaced_conditional_continuation(&mut self) {
        let keyword = self.advance();
        let name = if keyword.kind == TokenKind::Elif {
            "elif"
        } else {
            "else"
        };
        self.report(
            EXPECTED_STATEMENT,
            format!("`{name}` has no matching `if`"),
            keyword.span,
            format!("standalone `{name}` is not a statement"),
        );

        if keyword.kind == TokenKind::Elif {
            self.expect(TokenKind::LeftParen, "`(` after `elif`");
            if !self.at(TokenKind::RightParen) {
                let _ = self.parse_expression();
            }
            self.expect(TokenKind::RightParen, "`)` after the `elif` condition");
        }
        if self.at(TokenKind::LeftBrace) {
            let _ = self.parse_block();
        }
    }

    fn parse_local(&mut self) -> Option<LocalDecl> {
        let var_token = self.advance();
        let name = self.parse_name("expected a local name after `var`");
        self.expect(TokenKind::Colon, "`:` after the local name");
        let type_syntax = self.parse_type(
            TypeContext::LocalValue,
            format!(
                "expected the local type {}, a class name, or a shared object type",
                format_type_list(STORED_TYPE_NAMES)
            ),
        );
        self.expect(TokenKind::Equal, "`=` before the local initializer");
        let initializer = self.parse_expression();
        let semicolon = self.expect(TokenKind::Semicolon, "`;` after the local declaration");

        let (name, type_syntax, initializer) = match (name, type_syntax, initializer) {
            (Some(name), Some(type_syntax), Some(initializer)) => (name, type_syntax, initializer),
            _ => return None,
        };
        let end_span = semicolon
            .map(|token| token.span)
            .unwrap_or_else(|| initializer.span());

        Some(LocalDecl {
            name,
            type_syntax,
            initializer,
            span: self.cover(var_token.span, end_span),
        })
    }

    fn parse_return(&mut self) -> Option<ReturnStatement> {
        let return_token = self.advance();
        let value = (!self.at(TokenKind::Semicolon))
            .then(|| self.parse_expression())
            .flatten();
        let semicolon = self.expect(TokenKind::Semicolon, "`;` after the return statement");
        let end_span = semicolon
            .map(|token| token.span)
            .or_else(|| value.as_ref().map(Expression::span))
            .unwrap_or(return_token.span);

        Some(ReturnStatement {
            value,
            span: self.cover(return_token.span, end_span),
        })
    }

    fn parse_break(&mut self) -> BreakStatement {
        let break_token = self.advance();
        let semicolon = self.expect(TokenKind::Semicolon, "`;` after the `break` statement");
        let end_span = semicolon.map_or(break_token.span, |token| token.span);
        BreakStatement {
            break_span: break_token.span,
            span: self.cover(break_token.span, end_span),
        }
    }

    fn parse_continue(&mut self) -> ContinueStatement {
        let continue_token = self.advance();
        let semicolon = self.expect(TokenKind::Semicolon, "`;` after the `continue` statement");
        let end_span = semicolon.map_or(continue_token.span, |token| token.span);
        ContinueStatement {
            continue_span: continue_token.span,
            span: self.cover(continue_token.span, end_span),
        }
    }

    fn parse_expression_or_assignment(&mut self) -> Option<Statement> {
        let expression = self.parse_expression()?;
        if let Some(equal) = self.consume(TokenKind::Equal) {
            return self.parse_field_assignment(expression, equal);
        }

        let semicolon = self.expect(TokenKind::Semicolon, "`;` after the call expression");
        let end_span = semicolon
            .map(|token| token.span)
            .unwrap_or_else(|| expression.span());
        Some(Statement::Expression(ExpressionStatement {
            span: self.cover(expression.span(), end_span),
            expression,
        }))
    }

    fn parse_field_assignment(
        &mut self,
        expression: Expression,
        equal: Token,
    ) -> Option<Statement> {
        let value = self.parse_expression();
        let semicolon = self.expect(TokenKind::Semicolon, "`;` after the field assignment");
        let value = value?;
        let end_span = semicolon.map_or_else(|| value.span(), |token| token.span);

        if matches!(expression, Expression::ArrayProjection(_)) {
            return Some(Statement::ObjectAssignment(ObjectAssignmentStatement {
                span: self.cover(expression.span(), end_span),
                place: expression,
                equal_span: equal.span,
                value,
            }));
        }
        let Expression::MemberAccess(place) = expression else {
            if is_receiver_place(&expression) {
                return Some(Statement::ObjectAssignment(ObjectAssignmentStatement {
                    span: self.cover(expression.span(), end_span),
                    place: expression,
                    equal_span: equal.span,
                    value,
                }));
            }
            self.report(
                EXPECTED_STATEMENT,
                "only a place may be assigned",
                equal.span,
                "this expression does not have assignment-place syntax",
            );
            return None;
        };
        if !is_receiver_place(&place.receiver) {
            self.report(
                EXPECTED_STATEMENT,
                "invalid field-assignment receiver",
                place.receiver.span(),
                "expected an object-place path rooted at a local name or `self`",
            );
            return None;
        }

        Some(Statement::FieldAssignment(FieldAssignmentStatement {
            span: self.cover(place.span, end_span),
            place,
            equal_span: equal.span,
            value,
        }))
    }
}

fn is_receiver_place(expression: &Expression) -> bool {
    match expression {
        Expression::Identifier(_) | Expression::SelfValue(_) => true,
        Expression::Grouped(grouped) => is_receiver_place(&grouped.expression),
        Expression::MemberAccess(member) => is_receiver_place(&member.receiver),
        Expression::ArrayProjection(projection) => is_receiver_place(&projection.receiver),
        Expression::Unary(unary) if unary.operator == UnaryOperator::Dereference => true,
        Expression::NumericLiteral(_)
        | Expression::ByteLiteral(_)
        | Expression::StringLiteral(_)
        | Expression::Absent(_)
        | Expression::Present(_)
        | Expression::Boolean(_)
        | Expression::Unary(_)
        | Expression::Binary(_)
        | Expression::Logical(_)
        | Expression::TypeTest(_)
        | Expression::PresenceTest(_)
        | Expression::PrimitiveCast(_)
        | Expression::Allocation(_)
        | Expression::OptionalBoxAllocation(_)
        | Expression::ArrayConstruction(_)
        | Expression::Call(_) => false,
        Expression::ObjectCast(_) | Expression::Unwrap(_) => true,
    }
}
