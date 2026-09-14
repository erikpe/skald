//! Resolver-owned reactions to the shared source-order AST walk.
//!
//! The syntax walker owns structural child order. This visitor owns request
//! discovery policy: complete source types are closed where written, while an
//! unrequested generic declaration is pruned until specialization closes it
//! under substitution. In particular, a local's declared type is closed before
//! its initializer is scanned; binding scope remains with ordinary body
//! resolution and does not affect request discovery.

use super::syntax_type_closer::SyntaxTypeCloser;

use crate::syntax::{self, SyntaxNode, SyntaxVisitor, WalkControl};

pub(super) struct SourceRequestScanner<'resolver, 'semantic, 'interner, 'diagnostics, 'lookup> {
    resolver: SyntaxTypeCloser<'resolver, 'semantic, 'interner, 'diagnostics, 'lookup>,
}

impl<'resolver, 'semantic, 'interner, 'diagnostics, 'lookup>
    SourceRequestScanner<'resolver, 'semantic, 'interner, 'diagnostics, 'lookup>
{
    pub(super) fn new(
        resolver: SyntaxTypeCloser<'resolver, 'semantic, 'interner, 'diagnostics, 'lookup>,
    ) -> Self {
        Self { resolver }
    }

    pub(super) fn visit_unit(&mut self, unit: &syntax::CompilationUnit) {
        syntax::walk(unit, self);
    }
}

impl<'ast> SyntaxVisitor<'ast> for SourceRequestScanner<'_, '_, '_, '_, '_> {
    fn enter(&mut self, node: SyntaxNode<'ast>) -> WalkControl {
        match node {
            SyntaxNode::Declaration(syntax::TopLevelDeclaration::Class(class))
                if class.type_parameters.is_some() =>
            {
                WalkControl::Prune
            }
            SyntaxNode::Declaration(syntax::TopLevelDeclaration::Interface(interface))
                if interface.type_parameters.is_some() =>
            {
                WalkControl::Prune
            }
            SyntaxNode::Type(type_syntax) => {
                let _ = self.resolver.close(type_syntax);
                WalkControl::Prune
            }
            SyntaxNode::NamedType(type_syntax) => {
                let _ = self.resolver.close_named(type_syntax, false);
                WalkControl::Prune
            }
            SyntaxNode::Import(_) => WalkControl::Prune,
            SyntaxNode::CompilationUnit(_)
            | SyntaxNode::Declaration(_)
            | SyntaxNode::ClassMember(_)
            | SyntaxNode::Block(_)
            | SyntaxNode::Statement(_)
            | SyntaxNode::Expression(_) => WalkControl::Continue,
        }
    }

    fn leave(&mut self, _node: SyntaxNode<'ast>) {}
}
