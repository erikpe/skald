//! Explicit boundary between typed HIR and the currently representable MIR.

use crate::{
    hir::{
        HirBlock, HirCallArgument, HirExpression, HirExpressionKind, HirLocalInitializer,
        HirMethodCallTarget, HirObjectCall, HirObjectCallTarget, HirObjectProducer,
        HirObjectReturn, HirObjectSource, HirProgram, HirReturnValue, HirStatement,
    },
    source::Span,
};

use super::HirLoweringError;

pub(super) fn ensure_representable(hir: &HirProgram) -> Result<(), HirLoweringError> {
    let unsupported = hir
        .definitions
        .iter()
        .find_map(|definition| virtual_call_in_block(&definition.body))
        .or_else(|| {
            hir.class_definitions.iter().find_map(|class| {
                [
                    Some(&class.initializer),
                    class.copy_constructor.as_ref(),
                    class.copy_assignment.as_ref(),
                    class.destructor.as_ref(),
                ]
                .into_iter()
                .flatten()
                .chain(&class.methods)
                .find_map(|definition| virtual_call_in_block(&definition.body))
            })
        });
    match unsupported {
        Some(span) => Err(HirLoweringError::VirtualDispatchNotRepresented { span }),
        None => Ok(()),
    }
}

fn virtual_call_in_block(block: &HirBlock) -> Option<Span> {
    block.statements.iter().find_map(virtual_call_in_statement)
}

fn virtual_call_in_statement(statement: &HirStatement) -> Option<Span> {
    match statement {
        HirStatement::BaseInitialization(initialization) => {
            virtual_call_in_arguments(&initialization.arguments)
        }
        HirStatement::Local(local) => match &local.initializer {
            HirLocalInitializer::Value(value) => virtual_call_in_expression(value),
            HirLocalInitializer::Object(initialization) => {
                virtual_call_in_producer(&initialization.producer)
            }
            HirLocalInitializer::Copy(copy) => virtual_call_in_source(&copy.source),
        },
        HirStatement::Return(return_) => match &return_.value {
            Some(HirReturnValue::Scalar(value)) => virtual_call_in_expression(value),
            Some(HirReturnValue::Object(value)) => virtual_call_in_return(value),
            None => None,
        },
        HirStatement::Call(call) => virtual_call_in_expression(&call.call),
        HirStatement::Conditional(conditional) => conditional
            .arms
            .iter()
            .find_map(|arm| {
                virtual_call_in_expression(&arm.condition)
                    .or_else(|| virtual_call_in_block(&arm.body))
            })
            .or_else(|| {
                conditional
                    .else_block
                    .as_ref()
                    .and_then(virtual_call_in_block)
            }),
        HirStatement::Block(block) => virtual_call_in_block(block),
        HirStatement::FieldAssignment(assignment) => virtual_call_in_expression(&assignment.value),
        HirStatement::FieldConstruction(construction) => {
            virtual_call_in_arguments(&construction.construction.arguments)
        }
        HirStatement::FieldCopyConstruction(copy) => virtual_call_in_source(&copy.source),
        HirStatement::FieldCopyAssignment(copy) => virtual_call_in_source(&copy.source),
        HirStatement::CopyAssignment(copy) => virtual_call_in_source(&copy.source),
    }
}

fn virtual_call_in_expression(expression: &HirExpression) -> Option<Span> {
    match &expression.kind {
        HirExpressionKind::Unary { operand, .. } | HirExpressionKind::Grouped(operand) => {
            virtual_call_in_expression(operand)
        }
        HirExpressionKind::Binary { left, right, .. } => {
            virtual_call_in_expression(left).or_else(|| virtual_call_in_expression(right))
        }
        HirExpressionKind::DirectCall { arguments, .. } => virtual_call_in_arguments(arguments),
        HirExpressionKind::MethodCall {
            target, arguments, ..
        } => matches!(target, HirMethodCallTarget::Virtual { .. })
            .then_some(expression.span)
            .or_else(|| virtual_call_in_arguments(arguments)),
        HirExpressionKind::Binding(_)
        | HirExpressionKind::I64(_)
        | HirExpressionKind::U64(_)
        | HirExpressionKind::U8(_)
        | HirExpressionKind::F64Bits(_)
        | HirExpressionKind::Boolean(_)
        | HirExpressionKind::FieldRead(_) => None,
    }
}

fn virtual_call_in_arguments(arguments: &[HirCallArgument]) -> Option<Span> {
    arguments.iter().find_map(|argument| match argument {
        HirCallArgument::Value(value) => virtual_call_in_expression(value),
        HirCallArgument::Copy(copy) => virtual_call_in_source(&copy.source),
        HirCallArgument::Place(_) | HirCallArgument::View(_) => None,
    })
}

fn virtual_call_in_source(source: &HirObjectSource) -> Option<Span> {
    match source {
        HirObjectSource::Produced(producer) => virtual_call_in_producer(producer),
        HirObjectSource::Slice(slice) => virtual_call_in_source(&slice.source),
        HirObjectSource::Place(_) => None,
    }
}

fn virtual_call_in_producer(producer: &HirObjectProducer) -> Option<Span> {
    match producer {
        HirObjectProducer::Construct(construction) => {
            virtual_call_in_arguments(&construction.arguments)
        }
        HirObjectProducer::Call(call) => virtual_call_in_object_call(call),
    }
}

fn virtual_call_in_object_call(call: &HirObjectCall) -> Option<Span> {
    match &call.target {
        HirObjectCallTarget::Method {
            target: HirMethodCallTarget::Virtual { .. },
            ..
        } => Some(call.span),
        HirObjectCallTarget::Direct(_)
        | HirObjectCallTarget::Method {
            target: HirMethodCallTarget::Direct(_),
            ..
        } => virtual_call_in_arguments(&call.arguments),
    }
}

fn virtual_call_in_return(return_: &HirObjectReturn) -> Option<Span> {
    match return_ {
        HirObjectReturn::Copy { source, .. } => virtual_call_in_source(source),
        HirObjectReturn::Construct { construction, .. } => {
            virtual_call_in_arguments(&construction.arguments)
        }
    }
}
