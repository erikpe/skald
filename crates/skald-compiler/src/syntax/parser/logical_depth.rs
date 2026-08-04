//! Iterative logical-expression depth validation for path-sensitive phases.

use crate::syntax::ast::{
    ArrayConstructionArguments, ArrayProjectionBounds, CallArguments, Expression,
};

use super::MAX_LOGICAL_EXPRESSION_DEPTH;

pub(super) fn exceeds_limit(root: &Expression) -> bool {
    let mut pending = vec![(root, 0usize)];
    while let Some((expression, depth)) = pending.pop() {
        match expression {
            Expression::Absent(_)
            | Expression::Identifier(_)
            | Expression::NumericLiteral(_)
            | Expression::ByteLiteral(_)
            | Expression::StringLiteral(_)
            | Expression::Boolean(_)
            | Expression::SelfValue(_) => {}
            Expression::Unary(expression) => {
                pending.push((&expression.operand, depth));
            }
            Expression::Binary(expression) => {
                pending.push((&expression.left, depth));
                pending.push((&expression.right, depth));
            }
            Expression::Logical(expression) => {
                let logical_depth = depth + 1;
                if logical_depth > MAX_LOGICAL_EXPRESSION_DEPTH {
                    return true;
                }
                pending.push((&expression.left, logical_depth));
                pending.push((&expression.right, logical_depth));
            }
            Expression::TypeTest(expression) => {
                pending.push((&expression.source, depth));
            }
            Expression::PresenceTest(expression) => {
                pending.push((&expression.source, depth));
            }
            Expression::Unwrap(expression) => {
                pending.push((&expression.source, depth));
            }
            Expression::PrimitiveCast(expression) => {
                pending.push((&expression.source, depth));
            }
            Expression::ObjectCast(expression) => {
                pending.push((&expression.source, depth));
            }
            Expression::Allocation(expression) => {
                push_arguments(&expression.arguments, depth, &mut pending);
            }
            Expression::ArrayConstruction(expression) => match &expression.arguments {
                ArrayConstructionArguments::Empty { .. } => {}
                ArrayConstructionArguments::Length { length, .. } => {
                    pending.push((length, depth));
                }
                ArrayConstructionArguments::Copy { source, .. } => {
                    pending.push((source, depth));
                }
            },
            Expression::Call(expression) => {
                pending.push((&expression.callee, depth));
                push_arguments(&expression.arguments, depth, &mut pending);
            }
            Expression::Grouped(expression) => {
                pending.push((&expression.expression, depth));
            }
            Expression::MemberAccess(expression) => {
                pending.push((&expression.receiver, depth));
            }
            Expression::ArrayProjection(expression) => {
                pending.push((&expression.receiver, depth));
                match &expression.bounds {
                    ArrayProjectionBounds::Index(index) => {
                        pending.push((index, depth));
                    }
                    ArrayProjectionBounds::Slice { start, end, .. } => {
                        if let Some(start) = start {
                            pending.push((start, depth));
                        }
                        if let Some(end) = end {
                            pending.push((end, depth));
                        }
                    }
                }
            }
        }
    }
    false
}

fn push_arguments<'expression>(
    arguments: &'expression CallArguments,
    depth: usize,
    pending: &mut Vec<(&'expression Expression, usize)>,
) {
    match arguments {
        CallArguments::Ordinary(arguments) => {
            pending.extend(arguments.iter().map(|argument| (argument, depth)));
        }
        CallArguments::Copy { source, .. } => pending.push((source, depth)),
    }
}
