//! Recovering recursive-descent parser for the first vertical slice.

use crate::{
    diagnostics::{Diagnostic, Diagnostics},
    lexer::{Token, TokenKind},
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
        let mut functions = Vec::new();

        while !self.at(TokenKind::Eof) {
            if self.at(TokenKind::Invalid) {
                // The lexer already emitted the focused spelling diagnostic.
                self.advance();
                continue;
            }

            if !self.at(TokenKind::Fn) {
                self.report(
                    EXPECTED_DECLARATION,
                    "expected a function declaration",
                    self.peek().span,
                    "expected `fn` at file scope",
                );
                self.synchronize_declaration();
                continue;
            }

            if let Some(function) = self.parse_function() {
                functions.push(function);
            } else {
                self.synchronize_declaration();
            }
        }

        let ast = CompilationUnit {
            functions,
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
                        "trailing commas are not part of the M2 grammar",
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
        let type_syntax = self.parse_type("expected the parameter type `i64`");

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

        self.report(
            EXPECTED_TOKEN,
            message,
            self.peek().span,
            "the first vertical slice supports only `i64`",
        );
        if self.at(TokenKind::Identifier) {
            self.advance();
        }
        None
    }

    fn parse_block(&mut self) -> Option<Block> {
        let left_brace = self.expect(TokenKind::LeftBrace, "`{` to start a block")?;
        let mut statements = Vec::new();

        while !self.at_any(&[TokenKind::RightBrace, TokenKind::Eof]) {
            if self.at(TokenKind::Fn) {
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
        if self.at(TokenKind::LeftBrace) {
            return self.parse_block().map(Statement::Block);
        }

        self.report(
            EXPECTED_STATEMENT,
            "expected a statement",
            self.peek().span,
            "expected `var`, `return`, or a nested block",
        );
        None
    }

    fn parse_local(&mut self) -> Option<LocalDecl> {
        let var_token = self.advance();
        let name = self.parse_name("expected a local name after `var`");
        self.expect(TokenKind::Colon, "`:` after the local name");
        let type_syntax = self.parse_type("expected the local type `i64`");
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
        let value = self.parse_expression();
        let semicolon = self.expect(TokenKind::Semicolon, "`;` after the return value");
        let value = value?;
        let end_span = semicolon
            .map(|token| token.span)
            .unwrap_or_else(|| value.span());

        Some(ReturnStatement {
            value,
            span: self.cover(return_token.span, end_span),
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
                        "trailing commas are not part of the M2 grammar",
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

        if let Some(token) = self.consume(TokenKind::IntegerLiteral) {
            return Some(Expression::Integer(IntegerExpr {
                spelling: self.lexeme(token).to_owned(),
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
            "expected an identifier, decimal integer, unary `-`, or `(`",
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
        while !self.at_any(&[TokenKind::Fn, TokenKind::Eof]) {
            self.advance();
        }
    }

    fn synchronize_parameter(&mut self) {
        while !self.at_any(&[
            TokenKind::Comma,
            TokenKind::RightParen,
            TokenKind::Arrow,
            TokenKind::LeftBrace,
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
                TokenKind::LeftBrace,
                TokenKind::RightBrace,
                TokenKind::Fn,
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
            TokenKind::IntegerLiteral,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{lexer::lex, source::SourceDatabase, syntax::dump_ast};

    fn parse_text(text: &str) -> (SourceDatabase, ParseOutput) {
        let mut sources = SourceDatabase::new();
        let source_id = sources.add("test.ska", text);
        let source = sources.get(source_id).unwrap();
        let lexed = lex(source);
        assert!(lexed.diagnostics.is_empty(), "test source must lex cleanly");
        let parsed = parse(source, &lexed.tokens);
        (sources, parsed)
    }

    fn return_value(function: &FunctionDecl) -> &Expression {
        let Statement::Return(statement) = function.body.statements.last().unwrap() else {
            panic!("expected final return statement");
        };
        &statement.value
    }

    #[test]
    fn parses_the_vertical_slice_demonstration_program() {
        let source = concat!(
            "fn twice(value: i64) -> i64 {\n",
            "    return value * 2;\n",
            "}\n",
            "\n",
            "fn main() -> i64 {\n",
            "    var result: i64 = twice(20);\n",
            "    return result + 2;\n",
            "}\n",
        );
        let (_, output) = parse_text(source);

        assert!(!output.has_errors());
        assert_eq!(output.ast.functions.len(), 2);
        assert_eq!(output.ast.functions[0].name.text, "twice");
        assert_eq!(output.ast.functions[0].parameters.len(), 1);
        assert_eq!(output.ast.functions[1].name.text, "main");
        assert_eq!(output.ast.functions[1].body.statements.len(), 2);

        let Statement::Local(local) = &output.ast.functions[1].body.statements[0] else {
            panic!("expected local declaration");
        };
        let Expression::Call(call) = &local.initializer else {
            panic!("expected call initializer");
        };
        assert_eq!(call.arguments.len(), 1);
    }

    #[test]
    fn precedence_and_associativity_are_explicit() {
        let (_, output) = parse_text("fn main() -> i64 { return -a * b + c - d; }");
        assert!(!output.has_errors());

        let Expression::Binary(subtract) = return_value(&output.ast.functions[0]) else {
            panic!("outer expression must be subtraction");
        };
        assert_eq!(subtract.operator, BinaryOperator::Subtract);
        let Expression::Binary(add) = subtract.left.as_ref() else {
            panic!("subtraction left side must be addition");
        };
        assert_eq!(add.operator, BinaryOperator::Add);
        let Expression::Binary(multiply) = add.left.as_ref() else {
            panic!("addition left side must be multiplication");
        };
        assert_eq!(multiply.operator, BinaryOperator::Multiply);
        assert!(matches!(
            multiply.left.as_ref(),
            Expression::Unary(UnaryExpr {
                operator: UnaryOperator::Negate,
                ..
            })
        ));
    }

    #[test]
    fn grouping_overrides_binary_precedence_and_preserves_its_span() {
        let (_, output) = parse_text("fn main() -> i64 { return (1 + 2) * 3; }");
        let Expression::Binary(multiply) = return_value(&output.ast.functions[0]) else {
            panic!("expected multiplication");
        };
        let Expression::Grouped(grouped) = multiply.left.as_ref() else {
            panic!("expected grouped left operand");
        };
        assert_eq!(grouped.span.range().start(), 26);
        assert_eq!(grouped.span.range().end(), 33);
        assert!(matches!(
            grouped.expression.as_ref(),
            Expression::Binary(BinaryExpr {
                operator: BinaryOperator::Add,
                ..
            })
        ));
    }

    #[test]
    fn parser_does_not_perform_semantic_name_lookup() {
        let (_, output) =
            parse_text("fn main() -> i64 { var value: i64 = unknown(missing); return value; }");

        assert!(output.diagnostics.is_empty());
        assert_eq!(output.ast.functions.len(), 1);
    }

    #[test]
    fn malformed_function_does_not_hide_the_next_declaration() {
        let (_, output) = parse_text(concat!(
            "fn broken(value: Missing) -> i64 { return value; }\n",
            "fn main() -> i64 { return 0; }\n",
        ));

        assert!(output.has_errors());
        assert_eq!(output.ast.functions.len(), 1);
        assert_eq!(output.ast.functions[0].name.text, "main");
        assert!(!output.diagnostics.is_empty());
    }

    #[test]
    fn missing_punctuation_is_diagnosed_with_useful_recovery() {
        let (_, output) = parse_text(concat!(
            "fn main() -> i64 {\n",
            "    var first i64 = 1\n",
            "    var second: i64 = 2;\n",
            "    return first + second;\n",
            "}\n",
        ));

        assert!(output.has_errors());
        assert_eq!(output.diagnostics.len(), 2);
        assert_eq!(output.ast.functions.len(), 1);
        assert_eq!(output.ast.functions[0].body.statements.len(), 3);
        assert!(output
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code == EXPECTED_TOKEN));
    }

    #[test]
    fn independent_statement_errors_are_both_reported() {
        let (_, output) = parse_text(concat!(
            "fn main() -> i64 {\n",
            "    var : i64 = 1;\n",
            "    return ;\n",
            "    return 0;\n",
            "}\n",
        ));

        assert!(output.has_errors());
        assert!(output.diagnostics.len() >= 2);
        assert!(output.ast.functions[0]
            .body
            .statements
            .iter()
            .any(|statement| matches!(statement, Statement::Return(_))));
    }

    #[test]
    fn missing_block_close_recovers_at_the_next_function() {
        let (_, output) = parse_text(concat!(
            "fn first() -> i64 { return 1;\n",
            "fn second() -> i64 { return 2; }\n",
        ));

        assert!(output.has_errors());
        assert_eq!(output.ast.functions.len(), 2);
        assert_eq!(output.ast.functions[1].name.text, "second");
    }

    #[test]
    fn ast_dump_is_deterministic() {
        let (_, output) = parse_text("fn main() -> i64 { return add(1, -2); }");

        assert_eq!(
            dump_ast(&output.ast),
            concat!(
                "CompilationUnit @0..39\n",
                "  Function @0..39\n",
                "    Name \"main\" @3..7\n",
                "    Parameters\n",
                "    ReturnType\n",
                "      Type I64 @13..16\n",
                "    Block @17..39\n",
                "      Return @19..37\n",
                "        Call @26..36\n",
                "          Callee\n",
                "            Identifier \"add\" @26..29\n",
                "          Arguments\n",
                "            Integer \"1\" @30..31\n",
                "            Unary Negate @33..35\n",
                "              Integer \"2\" @34..35\n",
            )
        );
    }
}
