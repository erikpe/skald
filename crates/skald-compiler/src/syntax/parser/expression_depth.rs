//! Iterative expression-depth measurement for parser resource limits.

use crate::syntax::ast::{
    ArrayConstructionArguments, BracketProjectionBounds, CallArguments, Expression,
    OptionalBoxInitializer,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct Depths {
    pub(super) expression: usize,
    pub(super) logical: usize,
}

pub(super) fn measure(root: &Expression) -> Depths {
    let mut maximum = Depths::default();
    let mut pending = vec![(root, 1usize, 0usize)];

    while let Some((expression, expression_depth, logical_depth)) = pending.pop() {
        maximum.expression = maximum.expression.max(expression_depth);
        maximum.logical = maximum.logical.max(logical_depth);
        let child_depth = expression_depth + 1;

        match expression {
            Expression::Absent(_)
            | Expression::Identifier(_)
            | Expression::GenericTypeApplication(_)
            | Expression::GenericStaticSelection(_)
            | Expression::NumericLiteral(_)
            | Expression::ByteLiteral(_)
            | Expression::StringLiteral(_)
            | Expression::Boolean(_)
            | Expression::SelfValue(_) => {}
            Expression::Present(expression) => {
                pending.push((&expression.value, child_depth, logical_depth));
            }
            Expression::Unary(expression) => {
                pending.push((&expression.operand, child_depth, logical_depth));
            }
            Expression::Binary(expression) => {
                pending.push((&expression.left, child_depth, logical_depth));
                pending.push((&expression.right, child_depth, logical_depth));
            }
            Expression::Logical(expression) => {
                let child_logical_depth = logical_depth + 1;
                pending.push((&expression.left, child_depth, child_logical_depth));
                pending.push((&expression.right, child_depth, child_logical_depth));
            }
            Expression::TypeTest(expression) => {
                pending.push((&expression.source, child_depth, logical_depth));
            }
            Expression::PresenceTest(expression) => {
                pending.push((&expression.source, child_depth, logical_depth));
            }
            Expression::Unwrap(expression) => {
                pending.push((&expression.source, child_depth, logical_depth));
            }
            Expression::PrimitiveCast(expression) => {
                pending.push((&expression.source, child_depth, logical_depth));
            }
            Expression::ObjectCast(expression) => {
                pending.push((&expression.source, child_depth, logical_depth));
            }
            Expression::Allocation(expression) => {
                push_arguments(
                    &expression.arguments,
                    child_depth,
                    logical_depth,
                    &mut pending,
                );
            }
            Expression::OptionalBoxAllocation(expression) => {
                if let OptionalBoxInitializer::Value { value, .. } = &expression.initializer {
                    pending.push((value, child_depth, logical_depth));
                }
            }
            Expression::ArrayConstruction(expression) => match &expression.arguments {
                ArrayConstructionArguments::Empty { .. } => {}
                ArrayConstructionArguments::Length { length, .. } => {
                    pending.push((length, child_depth, logical_depth));
                }
                ArrayConstructionArguments::Copy { source, .. } => {
                    pending.push((source, child_depth, logical_depth));
                }
                ArrayConstructionArguments::Indexed(initializer) => {
                    pending.push((&initializer.length, child_depth, logical_depth));
                    pending.push((&initializer.element, child_depth, logical_depth));
                }
                ArrayConstructionArguments::Elements(list) => {
                    pending.extend(
                        list.elements
                            .iter()
                            .map(|element| (element, child_depth, logical_depth)),
                    );
                }
            },
            Expression::Call(expression) => {
                pending.push((&expression.callee, child_depth, logical_depth));
                push_arguments(
                    &expression.arguments,
                    child_depth,
                    logical_depth,
                    &mut pending,
                );
            }
            Expression::Grouped(expression) => {
                pending.push((&expression.expression, child_depth, logical_depth));
            }
            Expression::MemberAccess(expression) => {
                pending.push((&expression.receiver, child_depth, logical_depth));
            }
            Expression::BracketProjection(expression) => {
                pending.push((&expression.receiver, child_depth, logical_depth));
                match &expression.bounds {
                    BracketProjectionBounds::Index(index) => {
                        pending.push((index, child_depth, logical_depth));
                    }
                    BracketProjectionBounds::Slice { start, end, .. } => {
                        if let Some(start) = start {
                            pending.push((start, child_depth, logical_depth));
                        }
                        if let Some(end) = end {
                            pending.push((end, child_depth, logical_depth));
                        }
                    }
                }
            }
        }
    }

    maximum
}

fn push_arguments<'expression>(
    arguments: &'expression CallArguments,
    expression_depth: usize,
    logical_depth: usize,
    pending: &mut Vec<(&'expression Expression, usize, usize)>,
) {
    match arguments {
        CallArguments::Ordinary(arguments) => pending.extend(
            arguments
                .iter()
                .map(|argument| (argument, expression_depth, logical_depth)),
        ),
        CallArguments::Copy { source, .. } => {
            pending.push((source, expression_depth, logical_depth));
        }
    }
}
