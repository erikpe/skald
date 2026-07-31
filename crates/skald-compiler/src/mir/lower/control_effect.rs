//! Discovery of block-changing control-flow effects nested in HIR expressions.

use crate::hir::{
    HirCallArgument, HirCheckedObjectView, HirCheckedObjectViewKind, HirExpression,
    HirExpressionKind, HirInterfaceReceiver, HirObjectCallTarget, HirObjectProducer,
    HirObjectSource, HirSharedAllocationMode, HirSharedProducer, HirSharedSource, HirViewSource,
};

pub(super) fn expression_contains_control_effect(expression: &HirExpression) -> bool {
    match &expression.kind {
        HirExpressionKind::Unary { operand, .. } | HirExpressionKind::Grouped(operand) => {
            expression_contains_control_effect(operand)
        }
        HirExpressionKind::PrimitiveCast { operation, operand } => {
            operation.may_terminate() || expression_contains_control_effect(operand)
        }
        HirExpressionKind::Binary { left, right, .. }
        | HirExpressionKind::PrimitiveComparison { left, right, .. } => {
            expression_contains_control_effect(left) || expression_contains_control_effect(right)
        }
        // Checked arithmetic always introduces a semantic-check diamond,
        // regardless of whether its operands are otherwise pure.
        HirExpressionKind::CheckedIntegerDivision(_) | HirExpressionKind::CheckedShift(_) => true,
        // Logical expressions always select blocks even when both operands
        // are otherwise pure.
        HirExpressionKind::Logical(_) => true,
        HirExpressionKind::DirectCall { arguments, .. }
        | HirExpressionKind::StaticCall { arguments, .. } => {
            arguments.iter().any(call_argument_contains_control_effect)
        }
        HirExpressionKind::MethodCall {
            receiver,
            arguments,
            ..
        } => {
            method_receiver_contains_control_effect(receiver)
                || arguments.iter().any(call_argument_contains_control_effect)
        }
        HirExpressionKind::InterfaceCall {
            receiver,
            arguments,
            ..
        } => {
            interface_receiver_contains_control_effect(receiver)
                || arguments.iter().any(call_argument_contains_control_effect)
        }
        HirExpressionKind::FieldRead(place) => {
            place
                .checked_cast
                .as_deref()
                .is_some_and(checked_view_contains_control_effect)
                || place
                    .shared_view
                    .as_deref()
                    .is_some_and(|view| view_source_contains_control_effect(&view.source))
                || place
                    .optional_view
                    .as_deref()
                    .is_some_and(|view| view_source_contains_control_effect(&view.source))
                || place.array_element.is_some()
        }
        HirExpressionKind::TypeTest(test) => {
            view_source_contains_control_effect(&test.source.source)
        }
        HirExpressionKind::Binding(_)
        | HirExpressionKind::I64(_)
        | HirExpressionKind::U64(_)
        | HirExpressionKind::U8(_)
        | HirExpressionKind::F64Bits(_)
        | HirExpressionKind::Boolean(_)
        | HirExpressionKind::PresenceTest { .. } => false,
        HirExpressionKind::Unwrap(_) => true,
        HirExpressionKind::ArrayConstruction(construction) => {
            array_construction_contains_control_effect(construction)
        }
        // Checked array access lowers through explicit MIR control-flow
        // blocks, so earlier scalar values must be spilled before a later
        // operand changes the current block.
        HirExpressionKind::ArrayLength(_)
        | HirExpressionKind::ArrayElement(_)
        | HirExpressionKind::ArraySlice(_) => true,
    }
}

pub(super) fn call_argument_contains_control_effect(argument: &HirCallArgument) -> bool {
    match argument {
        HirCallArgument::Value(expression) => expression_contains_control_effect(expression),
        HirCallArgument::Optional { .. } => true,
        HirCallArgument::ClassOptional(_) => true,
        HirCallArgument::OptionalShared(_) => true,
        HirCallArgument::CheckedView(view) => checked_view_contains_control_effect(view),
        HirCallArgument::View(view) => view_source_contains_control_effect(&view.source),
        HirCallArgument::Copy(copy) => object_source_contains_control_effect(&copy.source),
        HirCallArgument::Place(_) | HirCallArgument::OptionalPlace(_) => false,
        HirCallArgument::Shared(transfer) => match &transfer.source {
            HirSharedSource::Produced(HirSharedProducer::Allocation(allocation)) => {
                shared_allocation_contains_control_effect(allocation)
            }
            HirSharedSource::Produced(HirSharedProducer::Call(call)) => {
                expression_contains_control_effect(call)
            }
            HirSharedSource::Produced(HirSharedProducer::Cast(cast)) => {
                cast.kind == crate::hir::HirSharedCastKind::RuntimeTerminate
                    || shared_source_contains_control_effect(&cast.source)
            }
            HirSharedSource::Produced(HirSharedProducer::OptionalUnwrap(_)) => true,
            HirSharedSource::Produced(HirSharedProducer::ArrayAllocation(construction)) => {
                array_construction_contains_control_effect(construction)
            }
            HirSharedSource::Place(_) => false,
        },
        HirCallArgument::Array(value) => array_source_contains_control_effect(&value.source),
        HirCallArgument::ArrayAlias(alias) => match &alias.source {
            crate::hir::HirArrayAliasSource::Whole(receiver) => {
                array_receiver_contains_control_effect(receiver)
            }
            crate::hir::HirArrayAliasSource::Element(_) => true,
        },
    }
}

fn shared_source_contains_control_effect(source: &HirSharedSource) -> bool {
    match source {
        HirSharedSource::Place(_) => false,
        HirSharedSource::Produced(HirSharedProducer::Allocation(allocation)) => {
            shared_allocation_contains_control_effect(allocation)
        }
        HirSharedSource::Produced(HirSharedProducer::Call(call)) => {
            expression_contains_control_effect(call)
        }
        HirSharedSource::Produced(HirSharedProducer::Cast(cast)) => {
            cast.kind == crate::hir::HirSharedCastKind::RuntimeTerminate
                || shared_source_contains_control_effect(&cast.source)
        }
        HirSharedSource::Produced(HirSharedProducer::OptionalUnwrap(_)) => true,
        HirSharedSource::Produced(HirSharedProducer::ArrayAllocation(construction)) => {
            array_construction_contains_control_effect(construction)
        }
    }
}

fn array_construction_contains_control_effect(
    construction: &crate::hir::HirArrayConstruction,
) -> bool {
    match &construction.mode {
        crate::hir::HirArrayConstructionMode::Empty => false,
        crate::hir::HirArrayConstructionMode::DefaultLength { length, .. } => {
            expression_contains_control_effect(length)
        }
        crate::hir::HirArrayConstructionMode::Copy { source, .. } => {
            array_source_contains_control_effect(source)
        }
    }
}

fn array_source_contains_control_effect(source: &crate::hir::HirArraySource) -> bool {
    array_receiver_contains_control_effect(&source.receiver)
}

fn array_receiver_contains_control_effect(receiver: &crate::hir::HirArrayReceiver) -> bool {
    match &receiver.source {
        crate::hir::HirArrayReceiverSource::Inline(expression) => {
            expression_contains_control_effect(expression)
        }
        crate::hir::HirArrayReceiverSource::Shared(source) => {
            shared_source_contains_control_effect(source)
        }
    }
}

fn shared_allocation_contains_control_effect(allocation: &crate::hir::HirSharedAllocation) -> bool {
    match &allocation.mode {
        HirSharedAllocationMode::Initialize { arguments, .. } => {
            arguments.iter().any(call_argument_contains_control_effect)
        }
        HirSharedAllocationMode::Copy { source, .. } => {
            object_source_contains_control_effect(source)
        }
    }
}

fn object_source_contains_control_effect(source: &HirObjectSource) -> bool {
    match source {
        HirObjectSource::Place(_) => false,
        HirObjectSource::ArrayElement(_) => false,
        HirObjectSource::Produced(producer) => producer_contains_control_effect(producer),
        HirObjectSource::Checked(view) => checked_view_contains_control_effect(view),
        HirObjectSource::Slice(slice) => object_source_contains_control_effect(&slice.source),
    }
}

fn checked_view_contains_control_effect(view: &HirCheckedObjectView) -> bool {
    view.kind == HirCheckedObjectViewKind::RuntimeTerminate
        || view_source_contains_control_effect(&view.view.source)
}

fn view_source_contains_control_effect(source: &HirViewSource) -> bool {
    match source {
        HirViewSource::Produced(producer) => producer_contains_control_effect(producer),
        HirViewSource::Place(_)
        | HirViewSource::Forwarded { .. }
        | HirViewSource::Shared { .. } => false,
        HirViewSource::AnchoredShared { source, .. } => {
            shared_source_contains_control_effect(source)
        }
        HirViewSource::OptionalPayload { .. } => true,
    }
}

fn producer_contains_control_effect(producer: &HirObjectProducer) -> bool {
    match producer {
        HirObjectProducer::StringLiteral(_) => false,
        HirObjectProducer::Construct(construction) => match &construction.mode {
            crate::hir::HirConstructionMode::Initialize { arguments, .. } => {
                arguments.iter().any(call_argument_contains_control_effect)
            }
            crate::hir::HirConstructionMode::Copy { source, .. } => {
                object_source_contains_control_effect(source)
            }
        },
        HirObjectProducer::Call(call) => {
            let receiver_has_cast = match &call.target {
                HirObjectCallTarget::Direct(_) | HirObjectCallTarget::Static(_) => false,
                HirObjectCallTarget::Method { receiver, .. } => {
                    method_receiver_contains_control_effect(receiver)
                }
                HirObjectCallTarget::Interface { receiver, .. } => {
                    interface_receiver_contains_control_effect(receiver)
                }
            };
            receiver_has_cast
                || call
                    .arguments
                    .iter()
                    .any(call_argument_contains_control_effect)
        }
    }
}

fn method_receiver_contains_control_effect(receiver: &crate::hir::HirMethodReceiver) -> bool {
    receiver
        .checked_cast
        .as_deref()
        .is_some_and(checked_view_contains_control_effect)
        || receiver
            .shared_view
            .as_deref()
            .is_some_and(|view| view_source_contains_control_effect(&view.source))
        || receiver
            .optional_view
            .as_deref()
            .is_some_and(|view| view_source_contains_control_effect(&view.source))
        || receiver.array_element.is_some()
}

fn interface_receiver_contains_control_effect(receiver: &HirInterfaceReceiver) -> bool {
    match receiver {
        HirInterfaceReceiver::View(view) => view_source_contains_control_effect(&view.source),
        HirInterfaceReceiver::Checked(view) => checked_view_contains_control_effect(view),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        hir::{
            HirBinaryOperation, HirCheckedIntegerDivision, HirIntegerDivisionKind,
            HirIntegerDivisionOperation, HirIntegerType, HirReturnValue, HirStatement,
        },
        test_support::type_check_source,
    };

    #[test]
    fn checked_integer_division_is_control_affecting_with_pure_operands() {
        let mut hir = type_check_source("fn main() -> i64 { return 8 + 3; }\n")
            .hir
            .unwrap();
        let definition = hir
            .definitions
            .get_mut_for_test(hir.entry_function)
            .unwrap();
        let HirStatement::Return(statement) = definition.body.statements.last_mut().unwrap() else {
            panic!("expected return statement");
        };
        let HirReturnValue::Scalar(expression) = statement.value.as_mut().unwrap() else {
            panic!("expected scalar return value");
        };
        let HirExpressionKind::Binary { left, right, .. } = &expression.kind else {
            panic!("expected binary expression");
        };
        expression.kind =
            HirExpressionKind::CheckedIntegerDivision(Box::new(HirCheckedIntegerDivision::new(
                HirIntegerDivisionOperation {
                    kind: HirIntegerDivisionKind::Quotient,
                    operand: HirIntegerType::I64,
                },
                (**left).clone(),
                (**right).clone(),
            )));

        assert!(expression_contains_control_effect(expression));
    }

    #[test]
    fn floating_division_is_not_control_affecting_with_pure_operands() {
        let mut hir = type_check_source(concat!(
            "fn divide() -> f64 { return 8.0 * 2.0; }\n",
            "fn main() -> i64 { return 0; }\n",
        ))
        .hir
        .unwrap();
        let definition = hir
            .definitions
            .get_mut_for_test(crate::identity::FunctionId::new(0))
            .unwrap();
        let HirStatement::Return(statement) = definition.body.statements.last_mut().unwrap() else {
            panic!("expected return statement");
        };
        let HirReturnValue::Scalar(expression) = statement.value.as_mut().unwrap() else {
            panic!("expected scalar return value");
        };
        let HirExpressionKind::Binary { operation, .. } = &mut expression.kind else {
            panic!("expected binary expression");
        };
        *operation = HirBinaryOperation::DivideF64;

        assert!(!expression_contains_control_effect(expression));
    }
}
