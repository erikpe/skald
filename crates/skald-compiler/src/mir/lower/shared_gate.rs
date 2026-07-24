//! Deliberate gate around the first executable shared-owner subset.

use std::collections::HashSet;

use crate::{
    hir::{
        HirBlock, HirCallArgument, HirLocal, HirLocalInitializer, HirOwnerTransfer, HirProgram,
        HirReturnValue, HirSharedProducer, HirSharedSource, HirSharedTarget, HirStatement, Type,
    },
    identity::LocalId,
    source::Span,
};

pub(super) fn first_unsupported_shared_span(hir: &HirProgram) -> Option<Span> {
    for declaration in hir.declarations.iter() {
        if matches!(declaration.return_type, Type::Shared(_)) {
            return Some(declaration.span);
        }
        if let Some(parameter) = declaration
            .parameters
            .iter()
            .find(|parameter| matches!(parameter.ty, Type::Shared(_)))
        {
            return Some(parameter.span);
        }
    }
    for class in hir.classes.iter() {
        if let Some(field) = class
            .fields
            .iter()
            .find(|field| matches!(field.ty, Type::Shared(_)))
        {
            return Some(field.span);
        }
        for initializer in &class.initializers {
            if let Some(parameter) = initializer
                .parameters
                .iter()
                .find(|parameter| matches!(parameter.ty, Type::Shared(_)))
            {
                return Some(parameter.span);
            }
        }
        for method in &class.methods {
            if matches!(method.return_type, Type::Shared(_)) {
                return Some(method.span);
            }
            if let Some(parameter) = method
                .parameters
                .iter()
                .find(|parameter| matches!(parameter.ty, Type::Shared(_)))
            {
                return Some(parameter.span);
            }
        }
    }
    for interface in hir.interfaces.iter() {
        for requirement in &interface.requirements {
            if matches!(requirement.return_type, Type::Shared(_)) {
                return Some(requirement.span);
            }
            if let Some(parameter) = requirement
                .parameters
                .iter()
                .find(|parameter| matches!(parameter.ty, Type::Shared(_)))
            {
                return Some(parameter.span);
            }
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
            HirStatement::Local(local) => match &local.initializer {
                HirLocalInitializer::Shared(transfer) => {
                    pending.remove(&local.local);
                    let HirSharedSource::Produced(HirSharedProducer::Allocation(allocation)) =
                        &transfer.source
                    else {
                        return Some(transfer.span);
                    };
                    if transfer.operation != HirOwnerTransfer::Adopt
                        || transfer.target != HirSharedTarget::Class(allocation.class)
                        || allocation
                            .arguments
                            .iter()
                            .any(|argument| matches!(argument, HirCallArgument::Shared(_)))
                    {
                        return Some(transfer.span);
                    }
                }
                HirLocalInitializer::Value(_)
                | HirLocalInitializer::Object(_)
                | HirLocalInitializer::Copy(_) => {}
            },
            HirStatement::Return(result)
                if matches!(result.value, Some(HirReturnValue::Shared(_))) =>
            {
                return Some(result.span);
            }
            HirStatement::SharedFieldWrite(write) => return Some(write.span),
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
            | HirStatement::Return(_)
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
