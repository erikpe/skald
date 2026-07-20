//! Recovering recursive-descent parser for the implemented source subset.

use crate::{
    diagnostics::{format_type_list, Diagnostic, Diagnostics},
    lexer::{Token, TokenKind},
    source::{SourceFile, Span},
};

use super::ast::*;

mod class;
mod declaration;
mod expression;
mod recovery;
mod statement;

pub const EXPECTED_DECLARATION: &str = "PAR001";
pub const EXPECTED_TOKEN: &str = "PAR002";
pub const EXPECTED_STATEMENT: &str = "PAR003";
pub const EXPECTED_EXPRESSION: &str = "PAR004";
pub const EXCESSIVE_NESTING: &str = "PAR005";
pub const INVALID_CLASS_MEMBER: &str = "PAR006";

/// Maximum number of simultaneously active recursive syntax constructs.
///
/// A function body consumes one level. Grouped and unary expressions, calls,
/// and nested blocks consume another level while their contents are parsed.
pub const MAX_SYNTAX_NESTING: usize = 128;

const STORED_TYPE_NAMES: &[&str] = &["i64", "u64", "u8", "f64", "bool"];
const RESULT_TYPE_NAMES: &[&str] = &["i64", "u64", "u8", "f64", "bool", "unit"];

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
    nesting_depth: usize,
    brace_depth: usize,
    class_depth: usize,
    recovering_from_excessive_nesting: bool,
}

impl<'source> Parser<'source> {
    fn new(source: &'source SourceFile, tokens: &'source [Token]) -> Self {
        Self {
            source,
            tokens,
            current: 0,
            diagnostics: Diagnostics::new(),
            nesting_depth: 0,
            brace_depth: 0,
            class_depth: 0,
            recovering_from_excessive_nesting: false,
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
            } else if self.at(TokenKind::Class) {
                self.parse_class().map(TopLevelDeclaration::Class)
            } else {
                self.report(
                    EXPECTED_DECLARATION,
                    "expected a top-level declaration",
                    self.peek().span,
                    "expected `fn`, `extern fn`, or `class` at file scope",
                );
                None
            };
            // An over-deep construct invalidates its entire declaration. This
            // keeps a partial recursive tree out of all downstream phases.
            let declaration = if self.recovering_from_excessive_nesting {
                None
            } else {
                declaration
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
        if self.recovering_from_excessive_nesting {
            return;
        }
        self.diagnostics
            .push(Diagnostic::error(code, message).with_primary_label(span, label));
    }

    /// Runs one recursively nested grammar operation within the shared parser
    /// budget. This is deliberately a counter rather than a heap-allocated
    /// recursion context, keeping the ordinary path to one comparison and two
    /// counter updates.
    fn with_syntax_nesting<T>(
        &mut self,
        span: Span,
        parse_nested: impl FnOnce(&mut Self) -> Option<T>,
    ) -> Option<T> {
        if self.nesting_depth >= MAX_SYNTAX_NESTING {
            self.report_excessive_nesting(span);
            self.recover_from_excessive_nesting();
            return None;
        }

        self.nesting_depth += 1;
        let result = parse_nested(self);
        self.nesting_depth -= 1;
        result
    }

    fn report_excessive_nesting(&mut self, span: Span) {
        if self.recovering_from_excessive_nesting {
            return;
        }

        self.diagnostics.push(
            Diagnostic::error(
                EXCESSIVE_NESTING,
                format!("syntax nesting exceeds the implementation limit of {MAX_SYNTAX_NESTING}"),
            )
            .with_primary_label(span, "this construct exceeds the nesting limit")
            .with_note("split deeply nested expressions or blocks into smaller statements"),
        );
        self.recovering_from_excessive_nesting = true;
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

    fn peek_ahead(&self, distance: usize) -> Token {
        self.tokens[(self.current + distance).min(self.tokens.len() - 1)]
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
