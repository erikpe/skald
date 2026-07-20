//! Recovering recursive-descent parser for the implemented source subset.

use crate::{
    diagnostics::{Diagnostic, Diagnostics},
    lexer::{Token, TokenKind},
    source::{SourceFile, Span},
};

use super::ast::*;

mod declaration;
mod expression;
mod recovery;
mod statement;

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

    fn parse_name(&mut self, message: &'static str) -> Option<Name> {
        let token = self.expect(TokenKind::Identifier, message)?;
        Some(Name {
            text: self.lexeme(token).to_owned(),
            span: token.span,
        })
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

    fn consume_numeric_literal(&mut self) -> Option<Token> {
        matches!(self.peek().kind, TokenKind::NumericLiteral(_)).then(|| self.advance())
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
