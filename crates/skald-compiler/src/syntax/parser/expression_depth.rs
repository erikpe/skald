//! Expression-depth measurement for parser resource limits.

use crate::syntax::{walk, Expression, SyntaxNode, SyntaxVisitor, WalkControl};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct Depths {
    pub(super) expression: usize,
    pub(super) logical: usize,
}

pub(super) fn measure(root: &Expression) -> Depths {
    let mut visitor = DepthVisitor::default();
    walk(root, &mut visitor);
    visitor.maximum
}

#[derive(Default)]
struct DepthVisitor {
    current: Depths,
    maximum: Depths,
}

impl<'ast> SyntaxVisitor<'ast> for DepthVisitor {
    fn enter(&mut self, node: SyntaxNode<'ast>) -> WalkControl {
        let SyntaxNode::Expression(expression) = node else {
            return WalkControl::Prune;
        };

        self.current.expression += 1;
        if matches!(expression, Expression::Logical(_)) {
            self.current.logical += 1;
        }
        self.maximum.expression = self.maximum.expression.max(self.current.expression);
        self.maximum.logical = self.maximum.logical.max(self.current.logical);
        WalkControl::Continue
    }

    fn leave(&mut self, node: SyntaxNode<'ast>) {
        let SyntaxNode::Expression(expression) = node else {
            return;
        };

        if matches!(expression, Expression::Logical(_)) {
            self.current.logical -= 1;
        }
        self.current.expression -= 1;
    }
}

#[cfg(test)]
mod tests;
