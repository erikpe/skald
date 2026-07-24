//! Deliberate gate around the executable exact-target shared-owner subset.

use std::collections::HashSet;

use crate::{
    hir::{
        HirBlock, HirCallArgument, HirLocal, HirLocalInitializer, HirOwnerTransfer, HirProgram,
        HirReturnValue, HirSharedPlace, HirSharedProducer, HirSharedSource, HirSharedTarget,
        HirSharedTransfer, HirStatement, Type,
    },
    identity::LocalId,
    source::Span,
};

pub(super) fn first_unsupported_shared_span(hir: &HirProgram) -> Option<Span> {
    for class in hir.classes.iter() {
        if let Some(field) = class
            .fields
            .iter()
            .find(|field| matches!(field.ty, Type::Shared(_)))
        {
            return Some(field.span);
        }
    }
    for definition in hir.definitions.iter() {
        if let Some(span) = validate_definition(&definition.locals, &definition.body) {
            return Some(span);
        }
    }
    for class in hir.class_definitions.iter() {
        for definition in class
            .initializers
            .iter()
            .chain(class.copy_constructor.iter())
            .chain(class.copy_assignment.iter())
            .chain(class.destructor.iter())
            .chain(class.methods.iter())
        {
            if let Some(span) = validate_definition(&definition.locals, &definition.body) {
                return Some(span);
            }
        }
    }
    None
}

fn validate_definition(locals: &[HirLocal], body: &HirBlock) -> Option<Span> {
    let mut pending: HashSet<_> = locals
        .iter()
        .filter(|local| matches!(local.ty, Type::Shared(_)))
        .map(|local| local.id)
        .collect();
    validate_block(body, &mut pending).or_else(|| {
        locals
            .iter()
            .find(|local| pending.contains(&local.id))
            .map(|local| local.span)
    })
}

fn validate_block(block: &HirBlock, pending: &mut HashSet<LocalId>) -> Option<Span> {
    for statement in &block.statements {
        match statement {
            HirStatement::BaseInitialization(initialization)
                if !arguments_support_shared(&initialization.arguments) =>
            {
                return Some(initialization.span);
            }
            HirStatement::Local(local) => match &local.initializer {
                HirLocalInitializer::Shared(transfer) => {
                    pending.remove(&local.local);
                    if !supports_exact_local_transfer(transfer) {
                        return Some(transfer.span);
                    }
                }
                HirLocalInitializer::Value(expression) => {
                    if !expression_supports_shared(expression) {
                        return Some(expression.span);
                    }
                }
                HirLocalInitializer::Object(initialization) => {
                    if !producer_supports_shared(&initialization.producer) {
                        return Some(initialization.span);
                    }
                }
                HirLocalInitializer::Copy(copy) => {
                    if !source_supports_shared(&copy.source) {
                        return Some(copy.span);
                    }
                }
            },
            HirStatement::Return(result) => match &result.value {
                Some(HirReturnValue::Shared(transfer)) if !supports_exact_transfer(transfer) => {
                    return Some(transfer.span);
                }
                Some(HirReturnValue::Scalar(expression))
                    if !expression_supports_shared(expression) =>
                {
                    return Some(expression.span);
                }
                Some(HirReturnValue::Object(result)) if !object_return_supports_shared(result) => {
                    return Some(result_span(result));
                }
                _ => {}
            },
            HirStatement::Call(call) if !expression_supports_shared(&call.call) => {
                return Some(call.span);
            }
            HirStatement::FieldAssignment(assignment)
                if !expression_supports_shared(&assignment.value) =>
            {
                return Some(assignment.span);
            }
            HirStatement::FieldConstruction(construction)
                if !construction_supports_shared(&construction.construction) =>
            {
                return Some(construction.span);
            }
            HirStatement::FieldCopyConstruction(copy) if !source_supports_shared(&copy.source) => {
                return Some(copy.span);
            }
            HirStatement::FieldCopyAssignment(copy) if !source_supports_shared(&copy.source) => {
                return Some(copy.span);
            }
            HirStatement::CopyAssignment(copy) if !source_supports_shared(&copy.source) => {
                return Some(copy.span);
            }
            HirStatement::SharedFieldWrite(write) => return Some(write.span),
            HirStatement::SharedAssignment(assignment) => {
                if !matches!(
                    assignment.destination,
                    crate::identity::BindingId::Local(_) | crate::identity::BindingId::Parameter(_)
                ) || !supports_exact_transfer(&assignment.value)
                {
                    return Some(assignment.span);
                }
            }
            HirStatement::Conditional(conditional) => {
                for arm in &conditional.arms {
                    if let Some(span) = validate_block(&arm.body, pending) {
                        return Some(span);
                    }
                }
                if let Some(else_block) = &conditional.else_block {
                    if let Some(span) = validate_block(else_block, pending) {
                        return Some(span);
                    }
                }
            }
            HirStatement::Block(block) => {
                if let Some(span) = validate_block(block, pending) {
                    return Some(span);
                }
            }
            HirStatement::BaseInitialization(_)
            | HirStatement::Call(_)
            | HirStatement::FieldAssignment(_)
            | HirStatement::FieldConstruction(_)
            | HirStatement::FieldCopyConstruction(_)
            | HirStatement::FieldCopyAssignment(_)
            | HirStatement::CopyAssignment(_) => {}
        }
    }
    None
}

fn supports_exact_local_transfer(transfer: &HirSharedTransfer) -> bool {
    supports_exact_transfer(transfer)
}

fn supports_exact_transfer(transfer: &HirSharedTransfer) -> bool {
    match &transfer.source {
        HirSharedSource::Place(HirSharedPlace::Binding { target, .. }) => {
            transfer.operation == HirOwnerTransfer::Copy && transfer.target == *target
        }
        HirSharedSource::Produced(HirSharedProducer::Allocation(allocation)) => {
            transfer.operation == HirOwnerTransfer::Adopt
                && transfer.target == HirSharedTarget::Class(allocation.class)
                && arguments_support_shared(&allocation.arguments)
        }
        HirSharedSource::Produced(HirSharedProducer::Call(call)) => {
            transfer.operation == HirOwnerTransfer::Adopt
                && transfer.target == transfer.source.target()
                && expression_supports_shared(call)
        }
        HirSharedSource::Place(HirSharedPlace::Field { .. }) => false,
    }
}

fn arguments_support_shared(arguments: &[HirCallArgument]) -> bool {
    arguments.iter().all(|argument| match argument {
        HirCallArgument::Value(expression) => expression_supports_shared(expression),
        HirCallArgument::Copy(copy) => source_supports_shared(&copy.source),
        HirCallArgument::Shared(transfer) => supports_exact_transfer(transfer),
        HirCallArgument::Place(_) | HirCallArgument::View(_) | HirCallArgument::CheckedView(_) => {
            true
        }
    })
}

fn expression_supports_shared(expression: &crate::hir::HirExpression) -> bool {
    match &expression.kind {
        crate::hir::HirExpressionKind::Unary { operand, .. }
        | crate::hir::HirExpressionKind::Grouped(operand) => expression_supports_shared(operand),
        crate::hir::HirExpressionKind::Binary { left, right, .. } => {
            expression_supports_shared(left) && expression_supports_shared(right)
        }
        crate::hir::HirExpressionKind::DirectCall { arguments, .. }
        | crate::hir::HirExpressionKind::MethodCall { arguments, .. }
        | crate::hir::HirExpressionKind::InterfaceCall { arguments, .. } => {
            arguments_support_shared(arguments)
        }
        _ => true,
    }
}

fn producer_supports_shared(producer: &crate::hir::HirObjectProducer) -> bool {
    match producer {
        crate::hir::HirObjectProducer::Construct(construction) => {
            construction_supports_shared(construction)
        }
        crate::hir::HirObjectProducer::Call(call) => arguments_support_shared(&call.arguments),
    }
}

fn construction_supports_shared(construction: &crate::hir::HirConstruction) -> bool {
    match &construction.mode {
        crate::hir::HirConstructionMode::Initialize { arguments, .. } => {
            arguments_support_shared(arguments)
        }
        crate::hir::HirConstructionMode::Copy { source, .. } => source_supports_shared(source),
    }
}

fn source_supports_shared(source: &crate::hir::HirObjectSource) -> bool {
    match source {
        crate::hir::HirObjectSource::Produced(producer) => producer_supports_shared(producer),
        crate::hir::HirObjectSource::Slice(slice) => source_supports_shared(&slice.source),
        crate::hir::HirObjectSource::Place(_) | crate::hir::HirObjectSource::Checked(_) => true,
    }
}

fn object_return_supports_shared(result: &crate::hir::HirObjectReturn) -> bool {
    match result {
        crate::hir::HirObjectReturn::Copy { source, .. } => source_supports_shared(source),
        crate::hir::HirObjectReturn::Construct { construction, .. } => {
            construction_supports_shared(construction)
        }
    }
}

fn result_span(result: &crate::hir::HirObjectReturn) -> Span {
    match result {
        crate::hir::HirObjectReturn::Copy { span, .. } => *span,
        crate::hir::HirObjectReturn::Construct { construction, .. } => construction.span,
    }
}
