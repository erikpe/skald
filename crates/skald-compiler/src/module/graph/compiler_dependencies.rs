//! Compiler-owned dependency evidence derived from valid parsed source.

use std::collections::BTreeMap;

use crate::{
    lexer::{Token, TokenKind},
    source::TextRange,
    syntax::{
        walk, CompilationUnit, ForInSource, Statement, SyntaxNode, SyntaxVisitor, WalkControl,
    },
};

use super::CompilerDependencyKind;

pub(super) fn collect(
    ast: &CompilationUnit,
    tokens: &[Token],
) -> BTreeMap<CompilerDependencyKind, Vec<TextRange>> {
    let mut dependencies = BTreeMap::<CompilerDependencyKind, Vec<TextRange>>::new();
    for token in tokens {
        let kind = match token.kind {
            TokenKind::StringLiteral => CompilerDependencyKind::StringLiteral,
            TokenKind::For => CompilerDependencyKind::GeneralIteration,
            _ => continue,
        };
        dependencies
            .entry(kind)
            .or_default()
            .push(token.span.range());
    }
    collect_range_sources(ast, &mut dependencies);
    dependencies
}

fn collect_range_sources(
    ast: &CompilationUnit,
    dependencies: &mut BTreeMap<CompilerDependencyKind, Vec<TextRange>>,
) {
    walk(ast, &mut RangeSourceCollector { dependencies });
}

struct RangeSourceCollector<'dependencies> {
    dependencies: &'dependencies mut BTreeMap<CompilerDependencyKind, Vec<TextRange>>,
}

impl<'ast> SyntaxVisitor<'ast> for RangeSourceCollector<'_> {
    fn enter(&mut self, node: SyntaxNode<'ast>) -> WalkControl {
        match node {
            SyntaxNode::Statement(Statement::ForIn(statement)) => {
                if let ForInSource::Range(range) = &statement.source {
                    self.dependencies
                        .entry(CompilerDependencyKind::RangeForSource)
                        .or_default()
                        .push(range.operator_span.range());
                }
                WalkControl::Continue
            }
            SyntaxNode::Expression(_)
            | SyntaxNode::Import(_)
            | SyntaxNode::Type(_)
            | SyntaxNode::NamedType(_) => WalkControl::Prune,
            SyntaxNode::CompilationUnit(_)
            | SyntaxNode::Declaration(_)
            | SyntaxNode::ClassMember(_)
            | SyntaxNode::Block(_)
            | SyntaxNode::Statement(_) => WalkControl::Continue,
        }
    }

    fn leave(&mut self, _node: SyntaxNode<'ast>) {}
}
