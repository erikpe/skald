//! Program metadata, declarations, and callable definition lowering.

use super::*;
use crate::hir::{
    HirAccess, HirClassDeclaration, HirCopyCapability, HirDestructionStep, HirFunctionDeclaration,
    HirFunctionDefinition, HirFunctionLinkage, HirMemberDefinition, HirMethodDispatch,
    HirSynthesizedFieldCopy,
};

pub(super) fn lower_program(hir: &HirProgram) -> MirProgram {
    let classes = hir.classes.iter().map(lower_class_declaration).collect();
    let declarations = hir.declarations.iter().map(lower_declaration).collect();
    let definitions = hir
        .declarations
        .iter()
        .map(|declaration| {
            hir.definitions
                .get(declaration.id)
                .map(|definition| lower_function_definition(declaration, definition))
        })
        .collect();
    let member_definitions = hir
        .class_definitions
        .iter()
        .flat_map(|class| {
            std::iter::once(&class.initializer)
                .chain(class.copy_constructor.iter())
                .chain(class.copy_assignment.iter())
                .chain(class.destructor.iter())
                .chain(class.methods.iter())
        })
        .map(|definition| lower_member_definition(hir, definition))
        .collect();
    MirProgram {
        classes: MirClassDeclarationTable::new(classes),
        virtual_families: MirVirtualFamilyTable::new(
            hir.virtual_families
                .iter()
                .map(|family| {
                    let members = std::iter::once(family.root)
                        .chain(
                            hir.classes
                                .iter()
                                .flat_map(|class| &class.methods)
                                .filter_map(|method| match method.dispatch {
                                    HirMethodDispatch::VirtualRoot {
                                        family: method_family,
                                        ..
                                    }
                                    | HirMethodDispatch::Override {
                                        family: method_family,
                                        ..
                                    } if method_family == family.id && method.id != family.root => {
                                        Some(method.id)
                                    }
                                    _ => None,
                                }),
                        )
                        .collect();
                    MirVirtualFamily {
                        id: family.id,
                        slot: family.slot,
                        root: family.root,
                        members,
                    }
                })
                .collect(),
        ),
        declarations: MirFunctionDeclarationTable::new(declarations),
        definitions: MirFunctionDefinitionTable::new(definitions),
        member_definitions: MirMemberDefinitionTable::new(member_definitions),
        entry_function: hir.entry_function,
        span: hir.span,
    }
}

fn lower_class_declaration(class: &HirClassDeclaration) -> MirClassDeclaration {
    let fields: Vec<_> = class
        .fields
        .iter()
        .map(|field| MirFieldDeclaration {
            id: field.id,
            name: field.name.clone(),
            ty: lower_type(field.ty),
            span: field.span,
        })
        .collect();
    let destructor = class
        .destructor
        .as_ref()
        .map(|destructor| MirDestructorDeclaration {
            id: destructor.id,
            receiver_access: match destructor.receiver_access {
                HirAccess::ReadOnly => MirReceiverAccess::ReadOnly,
                HirAccess::Mutable => MirReceiverAccess::Mutable,
            },
            span: destructor.span,
        });
    let destruction = MirDestructionPlan {
        destructor,
        steps: class
            .destruction
            .steps
            .iter()
            .map(|step| match *step {
                HirDestructionStep::UserBody(destructor) => {
                    MirDestructionStep::UserBody(destructor)
                }
                HirDestructionStep::Field(field) => MirDestructionStep::Field(field),
                HirDestructionStep::Base(base) => MirDestructionStep::Base(base),
            })
            .collect(),
    };
    MirClassDeclaration {
        id: class.id,
        name: class.name.clone(),
        direct_base: class.direct_base.as_ref().map(|base| MirDirectBase {
            class: base.class,
            span: base.span,
        }),
        fields,
        initializers: vec![MirInitializerDeclaration {
            id: class.initializer.id,
            parameters: class
                .initializer
                .parameters
                .iter()
                .map(lower_parameter)
                .collect(),
            span: class.initializer.span,
        }],
        copy_constructor_declaration: class.copy_constructor_declaration.as_ref().map(|copy| {
            MirInitializerDeclaration {
                id: copy.id,
                parameters: copy.parameters.iter().map(lower_parameter).collect(),
                span: copy.span,
            }
        }),
        copy_constructor: lower_copy_capability(&class.copy_constructor),
        copy_assignment_declaration: class.copy_assignment_declaration.as_ref().map(|copy| {
            MirCopyAssignmentDeclaration {
                id: copy.id,
                parameter: lower_parameter(&copy.parameter),
                span: copy.span,
            }
        }),
        copy_assignment: lower_copy_capability(&class.copy_assignment),
        destruction,
        methods: class
            .methods
            .iter()
            .map(|method| MirMethodDeclaration {
                id: method.id,
                name: method.name.clone(),
                receiver_access: match method.receiver_access {
                    HirAccess::ReadOnly => MirReceiverAccess::ReadOnly,
                    HirAccess::Mutable => MirReceiverAccess::Mutable,
                },
                parameters: method.parameters.iter().map(lower_parameter).collect(),
                return_type: lower_type(method.return_type),
                span: method.span,
            })
            .collect(),
        span: class.span,
    }
}

fn lower_copy_capability<I: Copy>(capability: &HirCopyCapability<I>) -> MirCopyCapability<I> {
    match capability {
        HirCopyCapability::User(copy) => MirCopyCapability::User(MirUserCopy {
            operation: copy.operation,
            base: copy.base.map(lower_base_copy),
        }),
        HirCopyCapability::Synthesized(copy) => {
            MirCopyCapability::Synthesized(MirSynthesizedCopy {
                class: copy.class,
                base: copy.base.map(lower_base_copy),
                fields: copy
                    .fields
                    .iter()
                    .map(|field| match *field {
                        HirSynthesizedFieldCopy::Primitive { field } => {
                            MirSynthesizedFieldCopy::Primitive { field }
                        }
                        HirSynthesizedFieldCopy::Class { field, operation } => {
                            MirSynthesizedFieldCopy::Class {
                                field,
                                operation: lower_selected_copy_operation(operation),
                            }
                        }
                    })
                    .collect(),
            })
        }
        HirCopyCapability::Unavailable => MirCopyCapability::Unavailable,
    }
}

fn lower_base_copy<I: Copy>(copy: crate::hir::HirBaseCopy<I>) -> MirBaseCopy<I> {
    MirBaseCopy {
        base: copy.base,
        operation: lower_selected_copy_operation(copy.operation),
    }
}

fn lower_declaration(declaration: &HirFunctionDeclaration) -> MirFunctionDeclaration {
    MirFunctionDeclaration {
        id: declaration.id,
        name: declaration.name.clone(),
        parameters: declaration.parameters.iter().map(lower_parameter).collect(),
        return_type: lower_type(declaration.return_type),
        linkage: match &declaration.linkage {
            HirFunctionLinkage::Internal => MirFunctionLinkage::Internal,
            HirFunctionLinkage::External { symbol } => MirFunctionLinkage::External {
                symbol: symbol.clone(),
            },
        },
        span: declaration.span,
    }
}

fn lower_function_definition(
    declaration: &HirFunctionDeclaration,
    definition: &HirFunctionDefinition,
) -> MirFunctionDefinition {
    let lowered = BodyLowerer::lower(BodyLoweringInput {
        callable: declaration.id.into(),
        parameters: &declaration.parameters,
        locals: &definition.locals,
        source_body: &definition.body,
        return_type: declaration.return_type,
        receiver_class: None,
    });
    MirFunctionDefinition {
        function: declaration.id,
        return_storage: lowered.return_storage,
        parameters: lowered.parameters,
        storage: lowered.storage,
        values: lowered.values,
        body: lowered.body,
        span: definition.span,
    }
}

fn lower_member_definition(
    hir: &HirProgram,
    definition: &HirMemberDefinition,
) -> MirMemberDefinition {
    let signature = hir
        .callable_signature(definition.callable)
        .expect("typed member definition must have a signature");
    let lowered = BodyLowerer::lower(BodyLoweringInput {
        callable: definition.callable,
        parameters: signature.parameters,
        locals: &definition.locals,
        source_body: &definition.body,
        return_type: signature.return_type,
        receiver_class: definition.callable.class(),
    });
    MirMemberDefinition {
        callable: definition.callable,
        return_storage: lowered.return_storage,
        receiver: lowered.receiver.expect("member body must lower a receiver"),
        parameters: lowered.parameters,
        storage: lowered.storage,
        values: lowered.values,
        body: lowered.body,
        span: definition.span,
    }
}

fn lower_parameter(parameter: &HirParameter) -> MirParameter {
    let ty = lower_type(parameter.ty);
    match parameter.mode {
        HirParameterMode::Value => MirParameter::value(ty),
        HirParameterMode::ReadOnlyAlias => MirParameter::read_only_alias(ty),
        HirParameterMode::MutableAlias => MirParameter::mutable_alias(ty),
    }
}
