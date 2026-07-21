use super::{object_fixtures::*, *};
use crate::{
    identity::{CallableId, ParameterId},
    passes::run_mir_pipeline,
};

#[derive(Clone, Copy)]
struct AliasFixtureIds {
    class: ClassId,
    observe: FunctionId,
    mutate: FunctionId,
    forward: FunctionId,
    initializer: InitializerId,
    method: MethodId,
}

#[test]
fn verifies_direct_member_initializer_forwarding_overlap_and_mixed_alias_arguments() {
    let (program, _) = alias_mir();

    assert!(verify_mir(&program).is_ok());
    let expected = program.clone();
    assert_eq!(run_mir_pipeline(program).unwrap(), expected);
}

#[test]
fn dump_exposes_modes_indirect_bases_and_ordered_argument_kinds() {
    let (program, ids) = alias_mir();
    let dump = dump_mir(&program);

    assert!(dump.contains(&format!("Declaration {} \"forward\" internal", ids.forward)));
    assert!(dump.contains(&format!(
        "Signature (ref class {}, mut ref class {}, i64) -> unit",
        ids.class, ids.class
    )));
    assert!(dump.contains("ref-parameter"));
    assert!(dump.contains("mut-ref-parameter"));
    assert!(dump.contains("place(indirect(f3:s0))"));
    assert!(dump.contains("value(f3:v0)"));
    assert_eq!(dump, dump_mir(&program));
    assert!(!dump.contains("offset"));
    assert!(!dump.contains("register"));
}

#[test]
fn rejects_parameter_mode_storage_and_external_signature_corruption() {
    let (mut wrong_storage, ids) = alias_mir();
    wrong_storage
        .definitions
        .get_mut_for_test(ids.observe)
        .unwrap()
        .storage[0]
        .kind = MirStorageKind::Parameter;
    assert!(messages(&wrong_storage)
        .iter()
        .any(|message| message.contains("storage mode differs from declaration")));

    let (mut primitive_alias, ids) = alias_mir();
    primitive_alias.declarations.entries_mut_for_test()[ids.observe.index()].parameters[0].ty =
        MirType::I64;
    primitive_alias
        .definitions
        .get_mut_for_test(ids.observe)
        .unwrap()
        .storage[0]
        .ty = MirType::I64;
    assert!(messages(&primitive_alias)
        .iter()
        .any(|message| message.contains("alias parameter 0 must have class type")));

    let (mut unlisted, ids) = alias_mir();
    unlisted
        .definitions
        .get_mut_for_test(ids.observe)
        .unwrap()
        .parameters
        .remove(0);
    let errors = messages(&unlisted);
    assert!(errors
        .iter()
        .any(|message| message.contains("definition has 1 parameters but declaration requires 2")));
    assert!(errors
        .iter()
        .any(|message| message.contains("is not listed by the definition")));

    let (mut external, ids) = alias_mir();
    let declaration = &mut external.declarations.entries_mut_for_test()[ids.observe.index()];
    declaration.linkage = MirFunctionLinkage::External {
        symbol: declaration.name.clone(),
    };
    external.definitions.remove_for_test(ids.observe);
    assert!(messages(&external).iter().any(|message| message
        .contains("external function cannot declare alias or object value parameters")));
}

#[test]
fn rejects_argument_kind_type_ownership_and_access_corruption() {
    let (mut kind, ids) = alias_mir();
    let entry = kind.entry_function;
    let call = direct_call_mut(&mut kind, ids.observe);
    call.arguments[0] = MirArgument::Value(ValueId::new(entry, 0));
    assert!(messages(&kind)
        .iter()
        .any(|message| message.contains("call argument 0 must be a place")));

    let (mut ty, ids) = alias_mir();
    let entry = ty.entry_function;
    let call = direct_call_mut(&mut ty, ids.observe);
    call.arguments[0] = MirArgument::Place(
        MirPlace::base(StorageId::new(entry, 0)).project_field(FieldId::new(ids.class, 0)),
    );
    assert!(messages(&ty)
        .iter()
        .any(|message| message.contains("call argument 0 type mismatch")));

    let (mut foreign, ids) = alias_mir();
    let call = direct_call_mut(&mut foreign, ids.observe);
    call.arguments[0] = MirArgument::Place(MirPlace::base(StorageId::new(ids.mutate, 0)));
    assert!(messages(&foreign)
        .iter()
        .any(|message| message.contains("is not declared in this function")));

    let (mut access, ids) = alias_mir();
    let function = access.definitions.get_mut_for_test(ids.forward).unwrap();
    let call = function.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call) if call.target == MirCallTarget::Direct(ids.mutate) => {
                Some(call)
            }
            _ => None,
        })
        .unwrap();
    call.arguments[0] = MirArgument::Place(MirPlace::alias_parameter(function.parameters[0]));
    assert!(messages(&access)
        .iter()
        .any(|message| message.contains("call argument 0 requires mutable access")));
}

#[test]
fn rejects_direct_alias_homes_readonly_writes_and_mutable_receiver_calls() {
    let (mut direct, ids) = alias_mir();
    let function = direct.definitions.get_mut_for_test(ids.observe).unwrap();
    let MirInstruction::Assign(load) = &mut function.body.blocks[0].instructions[0] else {
        panic!("expected alias load");
    };
    let MirRvalueKind::Load(place) = &mut load.rvalue.kind else {
        panic!("expected place load");
    };
    place.base = MirPlaceBase::Storage(function.parameters[0]);
    assert!(messages(&direct)
        .iter()
        .any(|message| message.contains("requires an indirect base")));

    let (mut readonly_store, ids) = alias_mir();
    let function = readonly_store
        .definitions
        .get_mut_for_test(ids.observe)
        .unwrap();
    function.body.blocks[0]
        .instructions
        .push(MirInstruction::Store(MirStore {
            destination: MirPlace::alias_parameter(function.parameters[0])
                .project_field(FieldId::new(ids.class, 0)),
            value: ValueId::new(ids.observe, 0),
            span: function.span,
        }));
    assert!(messages(&readonly_store)
        .iter()
        .any(|message| message.contains("store destination requires mutable access")));

    let (mut replacement, ids) = alias_mir();
    let function = replacement
        .definitions
        .get_mut_for_test(ids.mutate)
        .unwrap();
    function.body.blocks[0]
        .instructions
        .push(MirInstruction::Initialize(MirInitialize {
            destination: MirPlace::alias_parameter(function.parameters[0]),
            target: ids.initializer,
            arguments: vec![MirArgument::Place(MirPlace::alias_parameter(
                function.parameters[0],
            ))],
            span: function.span,
        }));
    assert!(messages(&replacement)
        .iter()
        .any(|message| message.contains("initializer destination must be owning storage")));

    let (mut receiver, ids) = alias_mir();
    receiver.classes.entries_mut_for_test()[ids.class.index()].methods[ids.method.index()]
        .receiver_access = MirReceiverAccess::Mutable;
    let function = receiver.definitions.get_mut_for_test(ids.forward).unwrap();
    function.body.blocks[0]
        .instructions
        .push(MirInstruction::Call(MirCall {
            target: MirCallTarget::Method(ids.method),
            receiver: Some(MirPlace::alias_parameter(function.parameters[0])),
            arguments: vec![
                MirArgument::Place(MirPlace::alias_parameter(function.parameters[0])),
                MirArgument::Place(MirPlace::alias_parameter(function.parameters[1])),
                MirArgument::Value(ValueId::new(ids.forward, 0)),
            ],
            result: None,
            span: function.span,
        }));
    assert!(messages(&receiver)
        .iter()
        .any(|message| message.contains("mutable method receiver requires mutable access")));
}

fn alias_mir() -> (MirProgram, AliasFixtureIds) {
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
            span,
        }),
        MirInstruction::Call(MirCall {
            target: MirCallTarget::Direct(mutate),
            receiver: None,
            arguments: vec![MirArgument::Place(MirPlace::base(
                object_ids.object_storage,
            ))],
            result: None,
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
            target: MirCallTarget::Method(method),
            receiver: Some(MirPlace::base(object_ids.object_storage)),
            arguments: vec![
                MirArgument::Place(MirPlace::base(object_ids.object_storage)),
                MirArgument::Place(MirPlace::base(second_object)),
                MirArgument::Value(scalar),
            ],
            result: None,
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
        receiver_access: MirReceiverAccess::ReadOnly,
        parameters: vec![
            MirParameter::read_only_alias(MirType::Class(class)),
            MirParameter::mutable_alias(MirType::Class(class)),
            MirParameter::value(MirType::I64),
        ],
        return_type: MirType::Unit,
        span,
    });
    let initializer_parameters = class_declaration.initializers[1].parameters.clone();
    let method_parameters = class_declaration.methods[1].parameters.clone();
    program.member_definitions = MirMemberDefinitionTable::new(vec![
        empty_member_definition(initializer.into(), class, &initializer_parameters, span),
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

fn declaration(
    id: FunctionId,
    name: &str,
    parameters: Vec<MirParameter>,
    span: crate::source::Span,
) -> MirFunctionDeclaration {
    MirFunctionDeclaration {
        id,
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
                span,
            }),
            MirInstruction::Call(MirCall {
                target: MirCallTarget::Direct(mutate),
                receiver: None,
                arguments: vec![MirArgument::Place(MirPlace::alias_parameter(
                    StorageId::new(id, 1),
                ))],
                result: None,
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
        receiver,
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

fn direct_call_mut(program: &mut MirProgram, target: FunctionId) -> &mut MirCall {
    program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap()
        .body
        .blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call) if call.target == MirCallTarget::Direct(target) => {
                Some(call)
            }
            _ => None,
        })
        .unwrap()
}
