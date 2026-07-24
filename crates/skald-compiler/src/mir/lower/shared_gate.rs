//! Detection of typed shared vocabulary before executable MIR support exists.

use crate::{
    hir::{HirProgram, Type},
    source::Span,
};

pub(super) fn first_shared_type_span(hir: &HirProgram) -> Option<Span> {
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
        if let Some(local) = definition
            .locals
            .iter()
            .find(|local| matches!(local.ty, Type::Shared(_)))
        {
            return Some(local.span);
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
            if let Some(local) = definition
                .locals
                .iter()
                .find(|local| matches!(local.ty, Type::Shared(_)))
            {
                return Some(local.span);
            }
        }
    }
    None
}
