//! Program metadata, declarations, and callable definition lowering.

use super::*;
use crate::hir::{
    HirAccess, HirClassDeclaration, HirCopyCapability, HirDestructionStep, HirFunctionDeclaration,
    HirFunctionDefinition, HirFunctionLinkage, HirInterfaceDeclaration, HirMemberDefinition,
    HirMethodDispatch, HirMethodKind, HirSynthesizedFieldCopy,
};

pub(super) fn lower_program(hir: &HirProgram) -> MirProgram {
    let string_language_item = hir
        .string_language_item
        .as_ref()
        .map(|item| lower_string_language_item(hir, item));
    let classes = hir.classes.iter().map(lower_class_declaration).collect();
    let declarations = hir.declarations.iter().map(lower_declaration).collect();
    let definitions = hir
        .declarations
        .iter()
        .map(|declaration| {
            hir.definitions.get(declaration.id).map(|definition| {
                lower_function_definition(
                    hir,
                    declaration,
                    definition,
                    string_language_item,
                    &hir.literal_data,
                )
            })
        })
        .collect();
    let member_definitions = hir
        .class_definitions
        .iter()
        .flat_map(|class| {
            class
                .initializers
                .iter()
                .chain(class.copy_constructor.iter())
                .chain(class.copy_assignment.iter())
                .chain(class.destructor.iter())
                .chain(class.methods.iter())
        })
        .map(|definition| lower_member_definition(hir, definition, string_language_item))
        .collect();
    MirProgram {
        modules: hir.modules.clone(),
        external_links: hir.external_links.clone(),
        function_types: MirFunctionTypeTable::new(
            hir.function_types
                .iter()
                .map(|function| MirFunctionType {
                    id: function.id,
                    parameters: function
                        .parameters
                        .iter()
                        .map(|parameter| MirParameter {
                            mode: match parameter.mode {
                                crate::hir::HirFunctionTypeParameterMode::Value => {
                                    MirParameterMode::Value
                                }
                                crate::hir::HirFunctionTypeParameterMode::ReadOnlyAlias => {
                                    MirParameterMode::ReadOnlyAlias
                                }
                                crate::hir::HirFunctionTypeParameterMode::MutableAlias => {
                                    MirParameterMode::MutableAlias
                                }
                            },
                            ty: optional_types::lower_type(parameter.ty),
                        })
                        .collect(),
                    result: optional_types::lower_type(function.result),
                    span: function.span,
                })
                .collect(),
        ),
        array_types: MirArrayTypeTable::new(hir.array_types.iter().map(lower_array_type).collect()),
        optional_types: optional_types::lower_optional_types(&hir.optional_types),
        optional_box_types: optional_box_types::lower(&hir.optional_box_types),
        string_language_item,
        literal_data: MirLiteralDataTable::new(
            hir.literal_data
                .iter()
                .map(|literal| {
                    let item = string_language_item
                        .expect("typed literal data requires string language-item metadata");
                    MirLiteralData {
                        id: literal.id,
                        bytes: literal.bytes.clone(),
                        array: item.storage_array,
                        length: u64::try_from(literal.bytes.len())
                            .expect("literal byte length must fit the language u64 length"),
                        mutability: MirStaticDataMutability::Immutable,
                        origin: MirStaticAllocationOrigin::Immortal,
                        span: literal.span,
                    }
                })
                .collect(),
        ),
        classes: MirClassDeclarationTable::new(classes),
        interfaces: MirInterfaceDeclarationTable::new(
            hir.interfaces
                .iter()
                .map(lower_interface_declaration)
                .collect(),
        ),
        virtual_families: MirVirtualFamilyTable::new(
            hir.virtual_families
                .iter()
                .map(|family| {
                    let members = std::iter::once(family.root)
                        .chain(
                            hir.classes
                                .iter()
                                .flat_map(|class| &class.methods)
                                .filter_map(|method| match method.kind.dispatch() {
                                    Some(HirMethodDispatch::VirtualRoot {
                                        family: method_family,
                                        ..
                                    })
                                    | Some(HirMethodDispatch::Override {
                                        family: method_family,
                                        ..
                                    }) if method_family == family.id
                                        && method.id != family.root =>
                                    {
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
        static_lifecycle: None,
        entry_function: hir.entry_function,
        span: hir.span,
    }
}

fn lower_string_language_item(
    hir: &HirProgram,
    item: &crate::hir::HirStringLanguageItem,
) -> MirStringLanguageItem {
    let storage = hir
        .field(item.storage_field)
        .expect("typed string language-item field must be declared");
    let Type::Shared(crate::hir::HirSharedTarget::Array(storage_array)) = storage.ty else {
        unreachable!("typed string storage field must be shared u8[]");
    };
    MirStringLanguageItem {
        class: item.class,
        storage_field: item.storage_field,
        start_field: item.start_field,
        length_field: item.length_field,
        hash_code_field: item.hash_code_field,
        storage_array,
    }
}

fn lower_array_type(array: &crate::hir::HirArrayType) -> MirArrayType {
    use crate::hir::{
        HirArrayAssignElement as A, HirArrayCopyElement as C, HirArrayDefaultElement as D,
        HirArrayDestroyElement as X,
    };
    MirArrayType {
        id: array.id,
        element: optional_types::lower_type(array.element),
        lifecycle: MirArrayLifecycle {
            default: array.lifecycle.default.map(|operation| match operation {
                D::Primitive => MirArrayDefaultElement::Primitive,
                D::OptionalAbsent => MirArrayDefaultElement::OptionalAbsent,
                D::Class { class, initializer } => {
                    MirArrayDefaultElement::Class { class, initializer }
                }
                D::ArrayEmpty(array) => MirArrayDefaultElement::ArrayEmpty(array),
                D::SharedClass { class, initializer } => {
                    MirArrayDefaultElement::SharedClass { class, initializer }
                }
                D::SharedArrayEmpty(array) => MirArrayDefaultElement::SharedArrayEmpty(array),
                D::SharedOptionalBoxAbsent(target) => {
                    MirArrayDefaultElement::SharedOptionalBoxAbsent(target)
                }
            }),
            copy: array.lifecycle.copy.map(|operation| match operation {
                C::Primitive => MirArrayCopyElement::Primitive,
                C::OptionalPrimitive => MirArrayCopyElement::OptionalPrimitive,
                C::Class { class, operation } => MirArrayCopyElement::Class {
                    class,
                    operation: lower_selected_copy_operation(operation),
                },
                C::OptionalClass { class, operation } => MirArrayCopyElement::OptionalClass {
                    class,
                    operation: lower_selected_copy_operation(operation),
                },
                C::Array(array) => MirArrayCopyElement::Array(array),
                C::Shared(target) => MirArrayCopyElement::Shared(lower_shared_target(target)),
                C::OptionalShared(target) => {
                    MirArrayCopyElement::OptionalShared(lower_shared_target(target))
                }
                C::Optional(optional) => MirArrayCopyElement::Optional(optional),
            }),
            assignment: array.lifecycle.assignment.map(|operation| match operation {
                A::Primitive => MirArrayAssignElement::Primitive,
                A::OptionalPrimitive => MirArrayAssignElement::OptionalPrimitive,
                A::Class { class, operation } => MirArrayAssignElement::Class {
                    class,
                    operation: lower_selected_copy_operation(operation),
                },
                A::OptionalClass {
                    class,
                    copy_constructor,
                    copy_assignment,
                } => MirArrayAssignElement::OptionalClass {
                    class,
                    copy_constructor: lower_selected_copy_operation(copy_constructor),
                    copy_assignment: lower_selected_copy_operation(copy_assignment),
                },
                A::Array(array) => MirArrayAssignElement::Array(array),
                A::Shared(target) => MirArrayAssignElement::Shared(lower_shared_target(target)),
                A::OptionalShared(target) => {
                    MirArrayAssignElement::OptionalShared(lower_shared_target(target))
                }
                A::Optional(optional) => MirArrayAssignElement::Optional(optional),
            }),
            destruction: match array.lifecycle.destruction {
                X::Trivial => MirArrayDestroyElement::Trivial,
                X::Class(class) => MirArrayDestroyElement::Class(class),
                X::OptionalClass(class) => MirArrayDestroyElement::OptionalClass(class),
                X::Array(array) => MirArrayDestroyElement::Array(array),
                X::Shared(target) => MirArrayDestroyElement::Shared(lower_shared_target(target)),
                X::OptionalShared(target) => {
                    MirArrayDestroyElement::OptionalShared(lower_shared_target(target))
                }
                X::Optional(optional) => MirArrayDestroyElement::Optional(optional),
            },
        },
    }
}

fn lower_class_declaration(class: &HirClassDeclaration) -> MirClassDeclaration {
    let fields: Vec<_> = class
        .fields
        .iter()
        .map(|field| MirFieldDeclaration {
            id: field.id,
            cell_span: field.cell_span,
            final_span: field.final_span,
            name: field.name.clone(),
            ty: optional_types::lower_type(field.ty),
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
                HirDestructionStep::SharedField(field) => MirDestructionStep::SharedField(field),
                HirDestructionStep::OptionalSharedField(field) => {
                    MirDestructionStep::OptionalSharedField(field)
                }
                HirDestructionStep::OptionalClassField(field) => {
                    MirDestructionStep::OptionalClassField(field)
                }
                HirDestructionStep::OptionalField { field, optional } => {
                    MirDestructionStep::OptionalField { field, optional }
                }
                HirDestructionStep::ArrayField(field) => MirDestructionStep::ArrayField(field),
                HirDestructionStep::Base(base) => MirDestructionStep::Base(base),
            })
            .collect(),
    };
    MirClassDeclaration {
        id: class.id,
        module: class.module,
        name: class.name.clone(),
        direct_base: class.direct_base.as_ref().map(|base| MirDirectBase {
            class: base.class,
            span: base.span,
        }),
        conformances: class
            .conformances
            .iter()
            .map(|conformance| MirInterfaceConformance {
                interface: conformance.interface,
                implementations: conformance
                    .implementations
                    .iter()
                    .map(|implementation| MirRequirementImplementation {
                        requirement: implementation.requirement,
                        method: implementation.method,
                    })
                    .collect(),
            })
            .collect(),
        fields,
        static_fields: class
            .static_fields
            .iter()
            .map(|field| MirStaticFieldDeclaration {
                id: field.id,
                final_span: field.final_span,
                name: field.name.clone(),
                ty: optional_types::lower_type(field.ty),
                initialization: field
                    .initializer
                    .as_ref()
                    .map_or(MirStaticFieldInitialization::ZeroDefault, |initializer| {
                        MirStaticFieldInitialization::Explicit(initializer.id)
                    }),
                lifecycle: None,
                span: field.span,
            })
            .collect(),
        initializers: class
            .initializers
            .iter()
            .map(|initializer| MirInitializerDeclaration {
                id: initializer.id,
                parameters: initializer.parameters.iter().map(lower_parameter).collect(),
                span: initializer.span,
            })
            .collect(),
        copy_constructor_declaration: class.copy_constructor_declaration.as_ref().map(|copy| {
            MirCopyConstructorDeclaration {
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
                kind: match method.kind {
                    HirMethodKind::Instance {
                        receiver_access: HirAccess::ReadOnly,
                        ..
                    } => MirMethodKind::Instance {
                        receiver_access: MirReceiverAccess::ReadOnly,
                    },
                    HirMethodKind::Instance {
                        receiver_access: HirAccess::Mutable,
                        ..
                    } => MirMethodKind::Instance {
                        receiver_access: MirReceiverAccess::Mutable,
                    },
                    HirMethodKind::Static => MirMethodKind::Static,
                },
                parameters: method.parameters.iter().map(lower_parameter).collect(),
                return_type: optional_types::lower_type(method.return_type),
                span: method.span,
            })
            .collect(),
        span: class.span,
    }
}

fn lower_interface_declaration(interface: &HirInterfaceDeclaration) -> MirInterfaceDeclaration {
    MirInterfaceDeclaration {
        id: interface.id,
        module: interface.module,
        name: interface.name.clone(),
        requirements: interface
            .requirements
            .iter()
            .map(|requirement| MirInterfaceRequirement {
                id: requirement.id,
                name: requirement.name.clone(),
                receiver_access: match requirement.receiver_access {
                    HirAccess::ReadOnly => MirReceiverAccess::ReadOnly,
                    HirAccess::Mutable => MirReceiverAccess::Mutable,
                },
                parameters: requirement
                    .parameters
                    .iter()
                    .map(|parameter| {
                        let ty = optional_types::lower_type(parameter.ty);
                        match parameter.mode {
                            crate::hir::HirParameterMode::Value => MirParameter::value(ty),
                            crate::hir::HirParameterMode::ReadOnlyAlias => {
                                MirParameter::read_only_alias(ty)
                            }
                            crate::hir::HirParameterMode::MutableAlias => {
                                MirParameter::mutable_alias(ty)
                            }
                        }
                    })
                    .collect(),
                return_type: optional_types::lower_type(requirement.return_type),
                span: requirement.span,
            })
            .collect(),
        span: interface.span,
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
                        HirSynthesizedFieldCopy::Scalar { field } => {
                            MirSynthesizedFieldCopy::Primitive { field }
                        }
                        HirSynthesizedFieldCopy::OptionalPrimitive { field, payload } => {
                            MirSynthesizedFieldCopy::OptionalPrimitive {
                                field,
                                payload: super::primitive::lower_primitive_type(payload),
                            }
                        }
                        HirSynthesizedFieldCopy::OptionalClass {
                            field,
                            class,
                            operation,
                        } => MirSynthesizedFieldCopy::OptionalClass {
                            field,
                            class,
                            operation: lower_selected_copy_operation(operation),
                        },
                        HirSynthesizedFieldCopy::Shared { field } => {
                            MirSynthesizedFieldCopy::Shared { field }
                        }
                        HirSynthesizedFieldCopy::OptionalShared { field, target } => {
                            MirSynthesizedFieldCopy::OptionalShared {
                                field,
                                target: lower_shared_target(target),
                            }
                        }
                        HirSynthesizedFieldCopy::Optional { field, optional } => {
                            MirSynthesizedFieldCopy::Optional { field, optional }
                        }
                        HirSynthesizedFieldCopy::Class { field, operation } => {
                            MirSynthesizedFieldCopy::Class {
                                field,
                                operation: lower_selected_copy_operation(operation),
                            }
                        }
                        HirSynthesizedFieldCopy::Array { field, array } => {
                            MirSynthesizedFieldCopy::Array { field, array }
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
        module: declaration.module,
        name: declaration.name.clone(),
        parameters: declaration.parameters.iter().map(lower_parameter).collect(),
        return_type: optional_types::lower_type(declaration.return_type),
        linkage: match &declaration.linkage {
            HirFunctionLinkage::Internal => MirFunctionLinkage::Internal,
            HirFunctionLinkage::External { link } => MirFunctionLinkage::External { link: *link },
            HirFunctionLinkage::Intrinsic { intrinsic } => MirFunctionLinkage::Intrinsic {
                intrinsic: *intrinsic,
            },
        },
        span: declaration.span,
    }
}

fn lower_function_definition(
    hir: &HirProgram,
    declaration: &HirFunctionDeclaration,
    definition: &HirFunctionDefinition,
    string_language_item: Option<MirStringLanguageItem>,
    literal_data: &crate::hir::HirLiteralDataTable,
) -> MirFunctionDefinition {
    let lowered = BodyLowerer::lower(BodyLoweringInput {
        callable: declaration.id.into(),
        parameters: &declaration.parameters,
        locals: &definition.locals,
        source_body: &definition.body,
        return_type: declaration.return_type,
        receiver_class: None,
        string_language_item,
        literal_data,
        array_types: &hir.array_types,
        optional_types: &hir.optional_types,
        optional_box_types: &hir.optional_box_types,
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
    string_language_item: Option<MirStringLanguageItem>,
) -> MirMemberDefinition {
    debug_assert_eq!(definition.callable.class(), Some(definition.class_owner));
    let signature = hir
        .callable_signature(definition.callable)
        .expect("typed member definition must have a signature");
    let lowered = BodyLowerer::lower(BodyLoweringInput {
        callable: definition.callable,
        parameters: signature.parameters,
        locals: &definition.locals,
        source_body: &definition.body,
        return_type: signature.return_type,
        receiver_class: definition.receiver_class,
        string_language_item,
        literal_data: &hir.literal_data,
        array_types: &hir.array_types,
        optional_types: &hir.optional_types,
        optional_box_types: &hir.optional_box_types,
    });
    MirMemberDefinition {
        callable: definition.callable,
        class_owner: definition.class_owner,
        return_storage: lowered.return_storage,
        receiver: lowered.receiver,
        parameters: lowered.parameters,
        storage: lowered.storage,
        values: lowered.values,
        body: lowered.body,
        span: definition.span,
    }
}

fn lower_parameter(parameter: &HirParameter) -> MirParameter {
    let ty = optional_types::lower_type(parameter.ty);
    match parameter.mode {
        HirParameterMode::Value => MirParameter::value(ty),
        HirParameterMode::ReadOnlyAlias => MirParameter::read_only_alias(ty),
        HirParameterMode::MutableAlias => MirParameter::mutable_alias(ty),
    }
}
