//! Discovery of runtime-cast control-flow edges nested in HIR expressions.

use crate::hir::{
    HirCallArgument, HirCheckedObjectView, HirCheckedObjectViewKind, HirExpression,
    HirExpressionKind, HirInterfaceReceiver, HirObjectCallTarget, HirObjectProducer,
    HirObjectSource, HirViewSource,
};

pub(super) fn expression_contains_runtime_cast(expression: &HirExpression) -> bool {
    match &expression.kind {
        HirExpressionKind::Unary { operand, .. } | HirExpressionKind::Grouped(operand) => {
            expression_contains_runtime_cast(operand)
        }
        HirExpressionKind::Binary { left, right, .. } => {
            expression_contains_runtime_cast(left) || expression_contains_runtime_cast(right)
        }
        HirExpressionKind::DirectCall { arguments, .. } => {
            arguments.iter().any(call_argument_contains_runtime_cast)
        }
        HirExpressionKind::MethodCall {
            receiver,
            arguments,
            ..
        } => {
            receiver
                .checked_cast
                .as_deref()
                .is_some_and(checked_view_contains_runtime_cast)
                || arguments.iter().any(call_argument_contains_runtime_cast)
        }
        HirExpressionKind::InterfaceCall {
            receiver,
            arguments,
            ..
        } => {
            interface_receiver_contains_runtime_cast(receiver)
                || arguments.iter().any(call_argument_contains_runtime_cast)
        }
        HirExpressionKind::FieldRead(place) => place
            .checked_cast
            .as_deref()
            .is_some_and(checked_view_contains_runtime_cast),
        HirExpressionKind::TypeTest(test) => view_source_contains_runtime_cast(&test.source.source),
        HirExpressionKind::Binding(_)
        | HirExpressionKind::I64(_)
        | HirExpressionKind::U64(_)
        | HirExpressionKind::U8(_)
        | HirExpressionKind::F64Bits(_)
        | HirExpressionKind::Boolean(_) => false,
    }
}

pub(super) fn call_argument_contains_runtime_cast(argument: &HirCallArgument) -> bool {
    match argument {
        HirCallArgument::Value(expression) => expression_contains_runtime_cast(expression),
        HirCallArgument::CheckedView(view) => checked_view_contains_runtime_cast(view),
        HirCallArgument::View(view) => view_source_contains_runtime_cast(&view.source),
        HirCallArgument::Copy(copy) => object_source_contains_runtime_cast(&copy.source),
        HirCallArgument::Place(_) => false,
    }
}

fn object_source_contains_runtime_cast(source: &HirObjectSource) -> bool {
    match source {
        HirObjectSource::Place(_) => false,
        HirObjectSource::Produced(producer) => producer_contains_runtime_cast(producer),
        HirObjectSource::Checked(view) => checked_view_contains_runtime_cast(view),
        HirObjectSource::Slice(slice) => object_source_contains_runtime_cast(&slice.source),
    }
}

fn checked_view_contains_runtime_cast(view: &HirCheckedObjectView) -> bool {
    view.kind == HirCheckedObjectViewKind::RuntimeTerminate
        || view_source_contains_runtime_cast(&view.view.source)
}

fn view_source_contains_runtime_cast(source: &HirViewSource) -> bool {
    match source {
        HirViewSource::Produced(producer) => producer_contains_runtime_cast(producer),
        HirViewSource::Place(_) | HirViewSource::Forwarded { .. } => false,
    }
}

fn producer_contains_runtime_cast(producer: &HirObjectProducer) -> bool {
    match producer {
        HirObjectProducer::Construct(construction) => match &construction.mode {
            crate::hir::HirConstructionMode::Initialize { arguments, .. } => {
                arguments.iter().any(call_argument_contains_runtime_cast)
            }
            crate::hir::HirConstructionMode::Copy { source, .. } => {
                object_source_contains_runtime_cast(source)
            }
        },
        HirObjectProducer::Call(call) => {
            let receiver_has_cast = match &call.target {
                HirObjectCallTarget::Direct(_) => false,
                HirObjectCallTarget::Method { receiver, .. } => receiver
                    .checked_cast
                    .as_deref()
                    .is_some_and(checked_view_contains_runtime_cast),
                HirObjectCallTarget::Interface { receiver, .. } => {
                    interface_receiver_contains_runtime_cast(receiver)
                }
            };
            receiver_has_cast
                || call
                    .arguments
                    .iter()
                    .any(call_argument_contains_runtime_cast)
        }
    }
}

fn interface_receiver_contains_runtime_cast(receiver: &HirInterfaceReceiver) -> bool {
    match receiver {
        HirInterfaceReceiver::View(view) => view_source_contains_runtime_cast(&view.source),
        HirInterfaceReceiver::Checked(view) => checked_view_contains_runtime_cast(view),
    }
}
