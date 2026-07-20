//! Recovering recursive-descent parser for the implemented source subset.

use crate::{
    diagnostics::{Diagnostic, Diagnostics},
    lexer::{Token, TokenKind},
    literal::NumericLiteralKind,
    source::{SourceFile, Span},
};

use super::ast::*;

pub const EXPECTED_DECLARATION: &str = "PAR001";
pub const EXPECTED_TOKEN: &str = "PAR002";
pub const EXPECTED_STATEMENT: &str = "PAR003";
pub const EXPECTED_EXPRESSION: &str = "PAR004";

#[derive(Debug)]
pub struct ParseOutput {
    pub ast: CompilationUnit,
    pub diagnostics: Diagnostics,
}

impl ParseOutput {
    pub fn has_errors(&self) -> bool {
        self.diagnostics.has_errors()
    }
}

/// Parses one lexer's token stream without performing name or type lookup.
///
/// Tokens are an internal phase boundary: they must all belong to `source` and
/// end in exactly the lexer's `Eof` sentinel. Violating that contract is a
/// compiler defect, while malformed Skald source is reported through
/// `ParseOutput::diagnostics`.
pub fn parse(source: &SourceFile, tokens: &[Token]) -> ParseOutput {
    assert!(
        !tokens.is_empty()
            && tokens
                .last()
                .is_some_and(|token| token.kind == TokenKind::Eof),
        "parser input must end with an EOF token"
    );
    assert!(
        tokens
            .iter()
            .all(|token| token.span.source_id() == source.id()),
        "parser tokens must belong to the supplied source"
    );

    Parser::new(source, tokens).parse()
}

struct Parser<'source> {
    source: &'source SourceFile,
    tokens: &'source [Token],
    current: usize,
    diagnostics: Diagnostics,
}

impl<'source> Parser<'source> {
    fn new(source: &'source SourceFile, tokens: &'source [Token]) -> Self {
        Self {
            source,
            tokens,
            current: 0,
            diagnostics: Diagnostics::new(),
        }
    }

    fn parse(mut self) -> ParseOutput {
        let mut declarations = Vec::new();

        while !self.at(TokenKind::Eof) {
            if self.at(TokenKind::Invalid) {
                // The lexer already emitted the focused spelling diagnostic.
                self.advance();
                continue;
            }

            let declaration = if self.at(TokenKind::Fn) {
                self.parse_function().map(TopLevelDeclaration::Function)
            } else if self.at(TokenKind::Extern) {
                self.parse_external_function()
                    .map(TopLevelDeclaration::ExternalFunction)
            } else {
                self.report(
                    EXPECTED_DECLARATION,
                    "expected a function declaration",
                    self.peek().span,
                    "expected `fn` or `extern fn` at file scope",
                );
                None
            };
            match declaration {
                Some(declaration) => declarations.push(declaration),
                None => self.synchronize_declaration(),
            }
        }

        let ast = CompilationUnit {
            declarations,
            span: self
                .source
                .span(0, self.source.len())
                .expect("complete source is always a valid span"),
        };

        ParseOutput {
            ast,
            diagnostics: self.diagnostics,
        }
    }

    fn parse_function(&mut self) -> Option<FunctionDecl> {
        let fn_token = self.advance();
        let name = self.parse_name("expected a function name after `fn`");
        let parameters = self.parse_parameter_list();
        self.expect(TokenKind::Arrow, "`->` after the parameter list");
        let return_type = self.parse_type("expected a return type after `->`");
        let body = self.parse_block();

        let (name, parameters, return_type, body) = match (name, parameters, return_type, body) {
            (Some(name), Some(parameters), Some(return_type), Some(body)) => {
                (name, parameters, return_type, body)
            }
            _ => return None,
        };
        let span = self.cover(fn_token.span, body.span);

        Some(FunctionDecl {
            name,
            parameters,
            return_type,
            body,
            span,
        })
    }

    fn parse_external_function(&mut self) -> Option<ExternalFunctionDecl> {
        let extern_token = self.advance();
        self.expect(TokenKind::Fn, "`fn` after `extern`")?;
        let name = self.parse_name("expected a function name after `extern fn`");
        let parameters = self.parse_parameter_list();
        self.expect(TokenKind::Arrow, "`->` after the parameter list");
        let return_type = self.parse_type("expected a return type after `->`");
        let semicolon = self.expect(
            TokenKind::Semicolon,
            "`;` after the external function declaration",
        );

        let (name, parameters, return_type) = match (name, parameters, return_type) {
            (Some(name), Some(parameters), Some(return_type)) => (name, parameters, return_type),
            _ => return None,
        };
        let end_span = semicolon
            .map(|token| token.span)
            .unwrap_or(return_type.span);
        Some(ExternalFunctionDecl {
            name,
            parameters,
            return_type,
            span: self.cover(extern_token.span, end_span),
        })
    }

    fn parse_parameter_list(&mut self) -> Option<Vec<Parameter>> {
        self.expect(TokenKind::LeftParen, "`(` after the function name");
        let mut parameters = Vec::new();
        let mut valid = true;

        if self.consume(TokenKind::RightParen).is_some() {
            return Some(parameters);
        }

        loop {
            if self.at_any(&[
                TokenKind::RightParen,
                TokenKind::Arrow,
                TokenKind::LeftBrace,
                TokenKind::Semicolon,
                TokenKind::Fn,
                TokenKind::Extern,
                TokenKind::Eof,
            ]) {
                break;
            }

            if let Some(parameter) = self.parse_parameter() {
                parameters.push(parameter);
            } else {
                valid = false;
                self.synchronize_parameter();
            }

            if self.consume(TokenKind::Comma).is_some() {
                if self.at(TokenKind::RightParen) {
                    self.report(
                        EXPECTED_TOKEN,
                        "expected a parameter after `,`",
                        self.peek().span,
                        "trailing commas are not supported",
                    );
                    valid = false;
                    break;
                }
                continue;
            }

            if self.at(TokenKind::Identifier) {
                self.report(
                    EXPECTED_TOKEN,
                    "expected `,` between parameters",
                    self.peek().span,
                    "insert `,` before this parameter",
                );
                continue;
            }

            break;
        }

        self.expect(TokenKind::RightParen, "`)` after the parameters");
        valid.then_some(parameters)
    }

    fn parse_parameter(&mut self) -> Option<Parameter> {
        let name = self.parse_name("expected a parameter name");
        self.expect(TokenKind::Colon, "`:` after the parameter name");
        let type_syntax = self.parse_value_type("expected the parameter type `i64` or `bool`");

        match (name, type_syntax) {
            (Some(name), Some(type_syntax)) => {
                let span = self.cover(name.span, type_syntax.span);
                Some(Parameter {
                    name,
                    type_syntax,
                    span,
                })
            }
            _ => None,
        }
    }

    fn parse_type(&mut self, message: &'static str) -> Option<TypeSyntax> {
        if let Some(token) = self.consume(TokenKind::I64) {
            return Some(TypeSyntax {
                kind: TypeKind::I64,
                span: token.span,
            });
        }
        if let Some(token) = self.consume(TokenKind::Bool) {
            return Some(TypeSyntax {
                kind: TypeKind::Bool,
                span: token.span,
            });
        }
        if let Some(token) = self.consume(TokenKind::Unit) {
            return Some(TypeSyntax {
                kind: TypeKind::Unit,
                span: token.span,
            });
        }

        self.report(
            EXPECTED_TOKEN,
            message,
            self.peek().span,
            "expected `i64`, `bool`, or `unit`",
        );
        if self.at(TokenKind::Identifier) {
            self.advance();
        }
        None
    }

    fn parse_value_type(&mut self, message: &'static str) -> Option<TypeSyntax> {
        if let Some(token) = self.consume(TokenKind::I64) {
            return Some(TypeSyntax {
                kind: TypeKind::I64,
                span: token.span,
            });
        }
        if let Some(token) = self.consume(TokenKind::Bool) {
            return Some(TypeSyntax {
                kind: TypeKind::Bool,
                span: token.span,
            });
        }

        self.report(
            EXPECTED_TOKEN,
            message,
            self.peek().span,
            "parameters and locals must have type `i64` or `bool`",
        );
        if self.at_any(&[TokenKind::Identifier, TokenKind::Unit]) {
            self.advance();
        }
        None
    }

    fn parse_block(&mut self) -> Option<Block> {
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
        if self.at(TokenKind::Var) {
            return self.parse_local().map(Statement::Local);
        }
        if self.at(TokenKind::Return) {
            return self.parse_return().map(Statement::Return);
        }
        if self.at(TokenKind::If) {
            return self.parse_conditional().map(Statement::Conditional);
        }
        if self.at_any(&[TokenKind::Elif, TokenKind::Else]) {
            self.parse_misplaced_conditional_continuation();
            return None;
        }
        if self.at(TokenKind::LeftBrace) {
            return self.parse_block().map(Statement::Block);
        }

        if self.starts_expression() {
            return self.parse_expression_statement().map(Statement::Expression);
        }

        self.report(
            EXPECTED_STATEMENT,
            "expected a statement",
            self.peek().span,
            "expected `var`, `return`, `if`, a call expression, or a nested block",
        );
        None
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
        let type_syntax = self.parse_value_type("expected the local type `i64` or `bool`");
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

    fn parse_expression_statement(&mut self) -> Option<ExpressionStatement> {
        let expression = self.parse_expression()?;
        let semicolon = self.expect(TokenKind::Semicolon, "`;` after the call expression");
        let end_span = semicolon
            .map(|token| token.span)
            .unwrap_or_else(|| expression.span());
        Some(ExpressionStatement {
            span: self.cover(expression.span(), end_span),
            expression,
        })
    }

    fn parse_expression(&mut self) -> Option<Expression> {
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

        if let Some(token) = self.consume(TokenKind::NumericLiteral(NumericLiteralKind::I64)) {
            return Some(Expression::NumericLiteral(NumericLiteralExpr {
                kind: NumericLiteralKind::I64,
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

    fn parse_name(&mut self, message: &'static str) -> Option<Name> {
        let token = self.expect(TokenKind::Identifier, message)?;
        Some(Name {
            text: self.lexeme(token).to_owned(),
            span: token.span,
        })
    }

    fn synchronize_declaration(&mut self) {
        while !self.at_any(&[TokenKind::Fn, TokenKind::Extern, TokenKind::Eof]) {
            self.advance();
        }
    }

    fn synchronize_parameter(&mut self) {
        while !self.at_any(&[
            TokenKind::Comma,
            TokenKind::RightParen,
            TokenKind::Arrow,
            TokenKind::LeftBrace,
            TokenKind::Semicolon,
            TokenKind::Fn,
            TokenKind::Extern,
            TokenKind::Eof,
        ]) {
            self.advance();
        }
    }

    fn synchronize_statement(&mut self) {
        while !self.at(TokenKind::Eof) {
            if self.consume(TokenKind::Semicolon).is_some() {
                return;
            }
            if self.at_any(&[
                TokenKind::Var,
                TokenKind::Return,
                TokenKind::If,
                TokenKind::Elif,
                TokenKind::Else,
                TokenKind::Identifier,
                TokenKind::NumericLiteral(NumericLiteralKind::I64),
                TokenKind::True,
                TokenKind::False,
                TokenKind::Minus,
                TokenKind::LeftParen,
                TokenKind::LeftBrace,
                TokenKind::RightBrace,
                TokenKind::Fn,
                TokenKind::Extern,
            ]) {
                return;
            }
            self.advance();
        }
    }

    fn synchronize_argument(&mut self) {
        while !self.at_any(&[
            TokenKind::Comma,
            TokenKind::RightParen,
            TokenKind::Semicolon,
            TokenKind::RightBrace,
            TokenKind::Eof,
        ]) {
            self.advance();
        }
    }

    fn starts_expression(&self) -> bool {
        self.at_any(&[
            TokenKind::Identifier,
            TokenKind::NumericLiteral(NumericLiteralKind::I64),
            TokenKind::True,
            TokenKind::False,
            TokenKind::Minus,
            TokenKind::LeftParen,
        ])
    }

    fn expect(&mut self, kind: TokenKind, expectation: &'static str) -> Option<Token> {
        if self.at(kind) {
            return Some(self.advance());
        }

        self.report(
            EXPECTED_TOKEN,
            format!("expected {expectation}"),
            self.peek().span,
            format!("found {}", self.peek().kind),
        );
        None
    }

    fn report(
        &mut self,
        code: &'static str,
        message: impl Into<String>,
        span: Span,
        label: impl Into<String>,
    ) {
        self.diagnostics
            .push(Diagnostic::error(code, message).with_primary_label(span, label));
    }

    fn consume(&mut self, kind: TokenKind) -> Option<Token> {
        self.at(kind).then(|| self.advance())
    }

    fn at(&self, kind: TokenKind) -> bool {
        self.peek().kind == kind
    }

    fn at_any(&self, kinds: &[TokenKind]) -> bool {
        kinds.iter().any(|kind| self.at(*kind))
    }

    fn peek(&self) -> Token {
        self.tokens[self.current.min(self.tokens.len() - 1)]
    }

    fn previous(&self) -> Token {
        self.tokens[self.current.saturating_sub(1)]
    }

    fn advance(&mut self) -> Token {
        let token = self.peek();
        if token.kind != TokenKind::Eof {
            self.current += 1;
        }
        token
    }

    fn lexeme(&self, token: Token) -> &str {
        self.source
            .slice(token.span.range())
            .expect("token range must be valid for its source")
    }

    fn cover(&self, start: Span, end: Span) -> Span {
        self.source
            .span(start.range().start(), end.range().end())
            .expect("syntax children must form a valid ordered source span")
    }
}
