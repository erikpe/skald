//! Immutable structural traversal of the source AST.
//!
//! This module owns only source-shaped parent/child relationships and event
//! order. Consumers retain all phase-specific state and meaning. Complete type
//! occurrences are opaque leaves so resolution and type processing keep their
//! existing ownership.

mod traversal;

use crate::source::Span;

use super::ast::{
    Block, ClassMember, CompilationUnit, Expression, ImportDeclaration, NamedTypeSyntax, Statement,
    TopLevelDeclaration, TypeSyntax,
};

pub(crate) use traversal::walk;

/// A borrowed source node observed during structural traversal.
#[derive(Clone, Copy, Debug)]
pub(crate) enum SyntaxNode<'ast> {
    CompilationUnit(&'ast CompilationUnit),
    Import(&'ast ImportDeclaration),
    Declaration(&'ast TopLevelDeclaration),
    ClassMember(&'ast ClassMember),
    Block(&'ast Block),
    Statement(&'ast Statement),
    Expression(&'ast Expression),
    Type(&'ast TypeSyntax),
    NamedType(&'ast NamedTypeSyntax),
}

impl SyntaxNode<'_> {
    /// Returns the complete source span retained by the underlying AST node.
    pub(crate) const fn span(self) -> Span {
        match self {
            Self::CompilationUnit(unit) => unit.span,
            Self::Import(import) => import.span(),
            Self::Declaration(declaration) => declaration.span(),
            Self::ClassMember(member) => member.span(),
            Self::Block(block) => block.span,
            Self::Statement(statement) => statement.span(),
            Self::Expression(expression) => expression.span(),
            Self::Type(type_syntax) => type_syntax.span,
            Self::NamedType(type_syntax) => type_syntax.span,
        }
    }
}

impl<'ast> From<&'ast CompilationUnit> for SyntaxNode<'ast> {
    fn from(unit: &'ast CompilationUnit) -> Self {
        Self::CompilationUnit(unit)
    }
}

impl<'ast> From<&'ast ImportDeclaration> for SyntaxNode<'ast> {
    fn from(import: &'ast ImportDeclaration) -> Self {
        Self::Import(import)
    }
}

impl<'ast> From<&'ast TopLevelDeclaration> for SyntaxNode<'ast> {
    fn from(declaration: &'ast TopLevelDeclaration) -> Self {
        Self::Declaration(declaration)
    }
}

impl<'ast> From<&'ast ClassMember> for SyntaxNode<'ast> {
    fn from(member: &'ast ClassMember) -> Self {
        Self::ClassMember(member)
    }
}

impl<'ast> From<&'ast Block> for SyntaxNode<'ast> {
    fn from(block: &'ast Block) -> Self {
        Self::Block(block)
    }
}

impl<'ast> From<&'ast Statement> for SyntaxNode<'ast> {
    fn from(statement: &'ast Statement) -> Self {
        Self::Statement(statement)
    }
}

impl<'ast> From<&'ast Expression> for SyntaxNode<'ast> {
    fn from(expression: &'ast Expression) -> Self {
        Self::Expression(expression)
    }
}

impl<'ast> From<&'ast TypeSyntax> for SyntaxNode<'ast> {
    fn from(type_syntax: &'ast TypeSyntax) -> Self {
        Self::Type(type_syntax)
    }
}

impl<'ast> From<&'ast NamedTypeSyntax> for SyntaxNode<'ast> {
    fn from(type_syntax: &'ast NamedTypeSyntax) -> Self {
        Self::NamedType(type_syntax)
    }
}

/// Controls whether traversal enters the current node's descendants.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WalkControl {
    Continue,
    Prune,
}

/// Receives balanced structural events from [`walk`].
pub(crate) trait SyntaxVisitor<'ast> {
    /// Observes `node` and decides whether its descendants should be visited.
    fn enter(&mut self, node: SyntaxNode<'ast>) -> WalkControl;

    /// Observes the end of `node`, including nodes pruned during [`Self::enter`].
    fn leave(&mut self, node: SyntaxNode<'ast>);
}

#[cfg(test)]
mod tests;
