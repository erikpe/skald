use super::{object_fixtures::*, *};
use crate::identity::{CallableId, ParameterId};

#[derive(Clone, Copy)]
pub(super) struct AliasFixtureIds {
    pub(super) class: ClassId,
    pub(super) observe: FunctionId,
    pub(super) mutate: FunctionId,
    pub(super) forward: FunctionId,
    pub(super) initializer: InitializerId,
    pub(super) method: MethodId,
}

pub(super) fn alias_mir() -> (MirProgram, AliasFixtureIds) {
    let (mut program, object_ids) = object_mir();
    let span = program.span;
    let class = object_ids.outer;
    let observe = FunctionId::new(1);
    let mutate = FunctionId::new(2);
    let forward = FunctionId::new(3);
    let initializer = InitializerId::new(class, 1);
    let method = MethodId::new(class, 1);

    let main_declaration = program
        .declarations
        .get(program.entry_function)
        .unwrap()
        .clone();
    program.declarations = MirFunctionDeclarationTable::new(vec![
        main_declaration,
        declaration(
            observe,
            "observe",
            vec![
                MirParameter::read_only_alias(MirType::Class(class)),
                MirParameter::value(MirType::I64),
            ],
            span,
        ),
        declaration(
            mutate,
            "mutate",
            vec![MirParameter::mutable_alias(MirType::Class(class))],
            span,
        ),
        declaration(
            forward,
            "forward",
            vec![
                MirParameter::read_only_alias(MirType::Class(class)),
                MirParameter::mutable_alias(MirType::Class(class)),
                MirParameter::value(MirType::I64),
            ],
            span,
        ),
    ]);

    let mut main = program
        .definitions
        .get(program.entry_function)
        .unwrap()
        .clone();
    main.body.blocks[0]
        .instructions
        .retain(|instruction| !matches!(instruction, MirInstruction::Cleanup(_)));
    let second_object = StorageId::new(program.entry_function, 1);
    main.storage.push(MirStorage {
        id: second_object,
        source: Some(BindingId::Local(LocalId::new(program.entry_function, 1))),
        name: "second".to_owned(),
        kind: MirStorageKind::Local,
        ty: MirType::Class(class),
        span,
    });
    let scalar = ValueId::new(program.entry_function, 0);
    main.body.blocks[0].instructions.extend([
        MirInstruction::Call(MirCall {
            target: MirCallTarget::Direct(observe),
            receiver: None,
            arguments: vec![
                MirArgument::Place(MirPlace::base(object_ids.object_storage)),
                MirArgument::Value(scalar),
            ],
            result: None,
            shared_result: None,
            destination: None,
            span,
        }),
        MirInstruction::Call(MirCall {
            target: MirCallTarget::Direct(mutate),
            receiver: None,
            arguments: vec![MirArgument::Place(MirPlace::base(
                object_ids.object_storage,
            ))],
            result: None,
            shared_result: None,
            destination: None,
            span,
        }),
        MirInstruction::Call(MirCall {
            target: MirCallTarget::Direct(forward),
            receiver: None,
            arguments: vec![
                MirArgument::Place(MirPlace::base(object_ids.object_storage)),
                MirArgument::Place(MirPlace::base(object_ids.object_storage)),
                MirArgument::Value(scalar),
            ],
            result: None,
            shared_result: None,
            destination: None,
            span,
        }),
        MirInstruction::Initialize(MirInitialize {
            destination: MirPlace::base(second_object),
            target: initializer,
            arguments: vec![MirArgument::Place(MirPlace::base(
                object_ids.object_storage,
            ))],
            span,
        }),
        MirInstruction::Call(MirCall {
            target: MirCallTarget::Method(MirMethodCallTarget::Direct(method)),
            receiver: Some(
                MirMethodReceiver::exact(
                    MirPlace::base(object_ids.object_storage),
                    class,
                    MirAliasAccess::Mutable,
                )
                .into(),
            ),
            arguments: vec![
                MirArgument::Place(MirPlace::base(object_ids.object_storage)),
                MirArgument::Place(MirPlace::base(second_object)),
                MirArgument::Value(scalar),
            ],
            result: None,
            shared_result: None,
            destination: None,
            span,
        }),
        MirInstruction::Cleanup(MirCleanup {
            destination: second_object.into(),
            target: class,
            span,
        }),
        MirInstruction::Cleanup(MirCleanup {
            destination: object_ids.object_storage.into(),
            target: class,
            span,
        }),
    ]);
    fixture_add_body_storage_lifetimes(&main.storage, &mut main.body, span);

    program.definitions = MirFunctionDefinitionTable::new(vec![
        Some(main),
        Some(observe_definition(
            observe,
            class,
            object_ids.outer_inner,
            object_ids.inner_value,
            span,
        )),
        Some(mutate_definition(
            mutate,
            class,
            object_ids.outer_inner,
            object_ids.inner_value,
            span,
        )),
        Some(forward_definition(forward, class, observe, mutate, span)),
    ]);

    let class_declaration = &mut program.classes.entries_mut_for_test()[class.index()];
    class_declaration
        .initializers
        .push(MirInitializerDeclaration {
            id: initializer,
            parameters: vec![MirParameter::read_only_alias(MirType::Class(class))],
            span,
        });
    class_declaration.methods.push(MirMethodDeclaration {
        id: method,
        name: "mix".to_owned(),
        kind: MirMethodKind::instance(MirReceiverAccess::ReadOnly),
        parameters: vec![
            MirParameter::read_only_alias(MirType::Class(class)),
            MirParameter::mutable_alias(MirType::Class(class)),
            MirParameter::value(MirType::I64),
        ],
        return_type: MirType::Unit,
        span,
    });
    let initializer_parameters = class_declaration.initializers[1].parameters.clone();
    let ordinary_initializer = class_declaration.initializers[0].clone();
    let ordinary_method = class_declaration.methods[0].clone();
    let method_parameters = class_declaration.methods[1].parameters.clone();
    program.member_definitions = MirMemberDefinitionTable::new(vec![
        fixture_empty_member_definition(
            ordinary_initializer.id.into(),
            class,
            &ordinary_initializer.parameters,
            span,
        ),
        empty_member_definition(initializer.into(), class, &initializer_parameters, span),
        getter_definition(
            ordinary_method.id,
            class,
            object_ids.outer_inner,
            object_ids.inner_value,
            span,
        ),
        empty_member_definition(method.into(), class, &method_parameters, span),
    ]);

    (
        program,
        AliasFixtureIds {
            class,
            observe,
            mutate,
            forward,
            initializer,
            method,
        },
    )
}

fn getter_definition(
    id: MethodId,
    class: ClassId,
    outer_inner: FieldId,
    inner_value: FieldId,
    span: crate::source::Span,
) -> MirMemberDefinition {
    let callable = CallableId::Method(id);
    let receiver = StorageId::new(callable, 0);
    let result = ValueId::new(callable, 0);
    MirMemberDefinition {
        callable,
        class_owner: class,
        return_storage: None,
        receiver: Some(receiver),
        parameters: vec![],
        storage: vec![MirStorage {
            id: receiver,
            source: Some(BindingId::Receiver(callable)),
            name: "self".to_owned(),
            kind: MirStorageKind::Receiver,
            ty: MirType::Class(class),
            span,
        }],
        values: vec![MirValue {
            id: result,
            ty: MirType::I64,
            span,
        }],
        body: MirBody {
            entry: BlockId::new(callable, 0),
            blocks: vec![MirBasicBlock {
                id: BlockId::new(callable, 0),
                instructions: vec![MirInstruction::Assign(MirAssignment {
                    result,
                    rvalue: MirRvalue {
                        kind: MirRvalueKind::Load(
                            MirPlace::base(receiver)
                                .project_field(outer_inner)
                                .project_field(inner_value),
                        ),
                        ty: MirType::I64,
                    },
                    span,
                })],
                terminator: Some(MirTerminator::Return {
                    value: Some(result),
                    span,
                }),
                span,
            }],
            path_conditions: vec![],
            logical_expressions: vec![],
        },
        span,
    }
}

fn declaration(
    id: FunctionId,
    name: &str,
    parameters: Vec<MirParameter>,
    span: crate::source::Span,
) -> MirFunctionDeclaration {
    MirFunctionDeclaration {
        id,
        module: crate::identity::ModuleId::new(0),
        name: name.to_owned(),
        parameters,
        return_type: MirType::Unit,
        linkage: MirFunctionLinkage::Internal,
        span,
    }
}

fn observe_definition(
    id: FunctionId,
    class: ClassId,
    outer_inner: FieldId,
    inner_value: FieldId,
    span: crate::source::Span,
) -> MirFunctionDefinition {
    let parameters = vec![
        alias_storage(id.into(), 0, 0, class, MirAliasAccess::ReadOnly, span),
        value_storage(id.into(), 1, 1, MirType::I64, span),
    ];
    let value = ValueId::new(id, 0);
    function_definition(
        id,
        parameters,
        vec![MirValue {
            id: value,
            ty: MirType::I64,
            span,
        }],
        vec![MirInstruction::Assign(MirAssignment {
            result: value,
            rvalue: MirRvalue {
                kind: MirRvalueKind::Load(
                    MirPlace::alias_parameter(StorageId::new(id, 0))
                        .project_field(outer_inner)
                        .project_field(inner_value),
                ),
                ty: MirType::I64,
            },
            span,
        })],
        span,
    )
}

fn mutate_definition(
    id: FunctionId,
    class: ClassId,
    outer_inner: FieldId,
    inner_value: FieldId,
    span: crate::source::Span,
) -> MirFunctionDefinition {
    let parameter = alias_storage(id.into(), 0, 0, class, MirAliasAccess::Mutable, span);
    let value = ValueId::new(id, 0);
    function_definition(
        id,
        vec![parameter],
        vec![MirValue {
            id: value,
            ty: MirType::I64,
            span,
        }],
        vec![
            MirInstruction::Assign(MirAssignment {
                result: value,
                rvalue: MirRvalue {
                    kind: MirRvalueKind::ConstantI64(1),
                    ty: MirType::I64,
                },
                span,
            }),
            MirInstruction::Store(MirStore {
                destination: MirPlace::alias_parameter(StorageId::new(id, 0))
                    .project_field(outer_inner)
                    .project_field(inner_value),
                value,
                authorization: None,
                final_authorization: None,
                span,
            }),
        ],
        span,
    )
}

fn forward_definition(
    id: FunctionId,
    class: ClassId,
    observe: FunctionId,
    mutate: FunctionId,
    span: crate::source::Span,
) -> MirFunctionDefinition {
    let storage = vec![
        alias_storage(id.into(), 0, 0, class, MirAliasAccess::ReadOnly, span),
        alias_storage(id.into(), 1, 1, class, MirAliasAccess::Mutable, span),
        value_storage(id.into(), 2, 2, MirType::I64, span),
    ];
    let value = ValueId::new(id, 0);
    function_definition(
        id,
        storage,
        vec![MirValue {
            id: value,
            ty: MirType::I64,
            span,
        }],
        vec![
            MirInstruction::Assign(MirAssignment {
                result: value,
                rvalue: MirRvalue {
                    kind: MirRvalueKind::Load(MirPlace::base(StorageId::new(id, 2))),
                    ty: MirType::I64,
                },
                span,
            }),
            MirInstruction::Call(MirCall {
                target: MirCallTarget::Direct(observe),
                receiver: None,
                arguments: vec![
                    MirArgument::Place(MirPlace::alias_parameter(StorageId::new(id, 0))),
                    MirArgument::Value(value),
                ],
                result: None,
                shared_result: None,
                destination: None,
                span,
            }),
            MirInstruction::Call(MirCall {
                target: MirCallTarget::Direct(mutate),
                receiver: None,
                arguments: vec![MirArgument::Place(MirPlace::alias_parameter(
                    StorageId::new(id, 1),
                ))],
                result: None,
                shared_result: None,
                destination: None,
                span,
            }),
        ],
        span,
    )
}

fn function_definition(
    id: FunctionId,
    storage: Vec<MirStorage>,
    values: Vec<MirValue>,
    instructions: Vec<MirInstruction>,
    span: crate::source::Span,
) -> MirFunctionDefinition {
    MirFunctionDefinition {
        function: id,
        return_storage: None,
        parameters: storage.iter().map(|storage| storage.id).collect(),
        storage,
        values,
        body: MirBody {
            entry: BlockId::new(id, 0),
            blocks: vec![MirBasicBlock {
                id: BlockId::new(id, 0),
                instructions,
                terminator: Some(MirTerminator::Return { value: None, span }),
                span,
            }],
            path_conditions: vec![],
            logical_expressions: vec![],
        },
        span,
    }
}

fn empty_member_definition(
    callable: CallableId,
    class: ClassId,
    parameters: &[MirParameter],
    span: crate::source::Span,
) -> MirMemberDefinition {
    let receiver = StorageId::new(callable, 0);
    let mut storage = vec![MirStorage {
        id: receiver,
        source: Some(BindingId::Receiver(callable)),
        name: "self".to_owned(),
        kind: MirStorageKind::Receiver,
        ty: MirType::Class(class),
        span,
    }];
    for (index, parameter) in parameters.iter().copied().enumerate() {
        storage.push(match parameter.mode {
            MirParameterMode::Value => {
                value_storage(callable, index + 1, index, parameter.ty, span)
            }
            MirParameterMode::ReadOnlyAlias => alias_storage(
                callable,
                index + 1,
                index,
                class,
                MirAliasAccess::ReadOnly,
                span,
            ),
            MirParameterMode::MutableAlias => alias_storage(
                callable,
                index + 1,
                index,
                class,
                MirAliasAccess::Mutable,
                span,
            ),
        });
    }
    MirMemberDefinition {
        callable,
        class_owner: callable
            .class()
            .expect("member fixture needs a class owner"),
        return_storage: None,
        receiver: Some(receiver),
        parameters: storage.iter().skip(1).map(|storage| storage.id).collect(),
        storage,
        values: vec![],
        body: MirBody {
            entry: BlockId::new(callable, 0),
            blocks: vec![MirBasicBlock {
                id: BlockId::new(callable, 0),
                instructions: vec![],
                terminator: Some(MirTerminator::Return { value: None, span }),
                span,
            }],
            path_conditions: vec![],
            logical_expressions: vec![],
        },
        span,
    }
}

fn alias_storage(
    callable: CallableId,
    storage_index: usize,
    parameter_index: usize,
    class: ClassId,
    access: MirAliasAccess,
    span: crate::source::Span,
) -> MirStorage {
    MirStorage {
        id: StorageId::new(callable, storage_index),
        source: Some(BindingId::Parameter(ParameterId::new(
            callable,
            parameter_index,
        ))),
        name: format!("alias{storage_index}"),
        kind: MirStorageKind::AliasParameter(access),
        ty: MirType::Class(class),
        span,
    }
}

fn value_storage(
    callable: CallableId,
    storage_index: usize,
    parameter_index: usize,
    ty: MirType,
    span: crate::source::Span,
) -> MirStorage {
    MirStorage {
        id: StorageId::new(callable, storage_index),
        source: Some(BindingId::Parameter(ParameterId::new(
            callable,
            parameter_index,
        ))),
        name: format!("value{parameter_index}"),
        kind: MirStorageKind::Parameter,
        ty,
        span,
    }
}
