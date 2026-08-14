use super::*;
use crate::identity::CallableId;

pub(super) struct AliasProgramIds {
    pub add: FunctionId,
    pub class: ClassId,
}

pub(super) fn alias_counter_program() -> (MirProgram, AliasProgramIds) {
    let mut program = counter_member_program();
    let span = program.span;
    let class = ClassId::new(0);
    let value_field = FieldId::new(class, 0);
    let alias_initializer = InitializerId::new(class, 1);
    let alias_method = MethodId::new(class, 3);
    let add = FunctionId::new(3);
    let forward = FunctionId::new(4);

    let mut declarations: Vec<_> = program.declarations.iter().cloned().collect();
    declarations.extend([
        unit_declaration(
            add,
            "alias_add",
            vec![
                MirParameter::mutable_alias(MirType::Class(class)),
                MirParameter::value(MirType::I64),
            ],
            span,
        ),
        unit_declaration(
            forward,
            "alias_forward",
            vec![
                MirParameter::mutable_alias(MirType::Class(class)),
                MirParameter::mutable_alias(MirType::Class(class)),
                MirParameter::value(MirType::I64),
            ],
            span,
        ),
    ]);
    program.declarations = MirFunctionDeclarationTable::new(declarations);

    let class_declaration = &mut program.classes.entries_mut_for_test()[class.index()];
    class_declaration.fields.extend([
        field(FieldId::new(class, 1), "padding1", MirType::I64, span),
        field(FieldId::new(class, 2), "padding2", MirType::I64, span),
        field(FieldId::new(class, 3), "padding3", MirType::I64, span),
    ]);
    class_declaration
        .initializers
        .push(MirInitializerDeclaration {
            id: alias_initializer,
            parameters: vec![MirParameter::read_only_alias(MirType::Class(class))],
            span,
        });
    class_declaration.methods.push(MirMethodDeclaration {
        id: alias_method,
        name: "add_from_alias".to_owned(),
        kind: MirMethodKind::instance(MirReceiverAccess::ReadOnly),
        parameters: vec![
            MirParameter::mutable_alias(MirType::Class(class)),
            MirParameter::value(MirType::F64),
            MirParameter::value(MirType::I64),
        ],
        return_type: MirType::Unit,
        span,
    });

    let mut main = program
        .definitions
        .get(program.entry_function)
        .unwrap()
        .clone();
    let first = StorageId::new(main.function, 0);
    let second = StorageId::new(main.function, 1);
    main.storage.push(MirStorage {
        id: second,
        source: Some(BindingId::Local(LocalId::new(main.function, 1))),
        name: "copy".to_owned(),
        kind: MirStorageKind::Local,
        ty: MirType::Class(class),
        span,
    });
    let float = ValueId::new(main.function, 4);
    main.values.push(MirValue {
        id: float,
        ty: MirType::F64,
        span,
    });
    let amount = ValueId::new(main.function, 2);
    let after_first_add = main.body.blocks[0]
        .instructions
        .iter()
        .position(|instruction| {
            matches!(
                instruction,
                MirInstruction::Call(MirCall {
                    target: MirCallTarget::Method(MirMethodCallTarget::Direct(method)),
                    ..
                }) if *method == MethodId::new(class, 0)
            )
        })
        .expect("counter fixture must contain its first add call")
        + 1;
    main.body.blocks[0].instructions.splice(
        after_first_add..after_first_add,
        [
            MirInstruction::Call(MirCall {
                target: MirCallTarget::Direct(add),
                receiver: None,
                arguments: vec![
                    MirArgument::Place(MirPlace::base(first)),
                    MirArgument::Value(amount),
                ],
                result: None,
                shared_result: None,
                destination: None,
                span,
            }),
            MirInstruction::Call(MirCall {
                target: MirCallTarget::Direct(forward),
                receiver: None,
                arguments: vec![
                    MirArgument::Place(MirPlace::base(first)),
                    MirArgument::Place(MirPlace::base(first)),
                    MirArgument::Value(amount),
                ],
                result: None,
                shared_result: None,
                destination: None,
                span,
            }),
            MirInstruction::Initialize(MirInitialize {
                destination: MirPlace::base(second),
                target: alias_initializer,
                arguments: vec![MirArgument::Place(MirPlace::base(first))],
                span,
            }),
            assignment(
                main.function,
                4,
                MirRvalueKind::ConstantF64Bits(1.5_f64.to_bits()),
                MirType::F64,
                span,
            ),
            MirInstruction::Call(MirCall {
                target: MirCallTarget::Method(MirMethodCallTarget::Direct(alias_method)),
                receiver: Some(
                    MirMethodReceiver::exact(MirPlace::base(first), class, MirAliasAccess::Mutable)
                        .into(),
                ),
                arguments: vec![
                    MirArgument::Place(MirPlace::base(second)),
                    MirArgument::Value(float),
                    MirArgument::Value(amount),
                ],
                result: None,
                shared_result: None,
                destination: None,
                span,
            }),
        ],
    );
    let get_call = main.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call)
                if call.target
                    == MirCallTarget::Method(MirMethodCallTarget::Direct(MethodId::new(
                        class, 2,
                    ))) =>
            {
                Some(call)
            }
            _ => None,
        })
        .expect("counter fixture must read through its forwarding getter");
    get_call.receiver = Some(
        MirMethodReceiver::exact(MirPlace::base(second), class, MirAliasAccess::Mutable).into(),
    );
    let first_cleanup = main.body.blocks[0]
        .instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::Cleanup(_)))
        .expect("counter fixture must clean its owning local");
    main.body.blocks[0].instructions.insert(
        first_cleanup,
        MirInstruction::Cleanup(MirCleanup {
            destination: second.into(),
            target: class,
            span,
        }),
    );
    fixture_add_body_storage_lifetimes(&main.storage, &mut main.body, span);

    let sum = FunctionId::new(1);
    let existing_sum = program.definitions.get(sum).unwrap().clone();
    program.definitions = MirFunctionDefinitionTable::new(vec![
        None,
        Some(existing_sum),
        Some(main),
        Some(alias_add_definition(add, class, value_field, sum, span)),
        Some(alias_forward_definition(forward, class, add, span)),
    ]);

    let mut members: Vec<_> = program.member_definitions.iter().cloned().collect();
    members.extend([
        alias_initializer_definition(alias_initializer, value_field, span),
        alias_method_definition(alias_method, class, add, span),
    ]);
    program.member_definitions = MirMemberDefinitionTable::new(members);

    (program, AliasProgramIds { add, class })
}

pub(super) fn alias_record_i64_stub() -> &'static str {
    concat!(
        ".section .rodata\n",
        ".Lalias_output:\n",
        "    .ascii \"50\\n\"\n",
        ".text\n",
        ".globl test_record_i64\n",
        ".type test_record_i64, @function\n",
        "test_record_i64:\n",
        "    cmp rdi, 50\n",
        "    jne .Lalias_bad_value\n",
        "    mov rax, 1\n",
        "    mov rdi, 1\n",
        "    lea rsi, [rip + .Lalias_output]\n",
        "    mov rdx, 3\n",
        "    syscall\n",
        "    ret\n",
        ".Lalias_bad_value:\n",
        "    mov rax, 60\n",
        "    mov rdi, 99\n",
        "    syscall\n",
        ".size test_record_i64, .-test_record_i64\n",
    )
}

pub(super) fn exhausted_receiver_alias_abi_program() -> MirProgram {
    let (mut program, ids) = projected_object_program();
    let span = program.span;
    let initializer = InitializerId::new(ids.container, 0);
    let method = MethodId::new(ids.container, 0);
    let mut parameters = vec![MirParameter::read_only_alias(MirType::Class(ids.container)); 5];
    parameters.extend(MirParameter::values([MirType::F64; 8]));
    parameters.extend([
        MirParameter::mutable_alias(MirType::Class(ids.container)),
        MirParameter::value(MirType::F64),
    ]);
    let class = &mut program.classes.entries_mut_for_test()[ids.container.index()];
    class.initializers.push(MirInitializerDeclaration {
        id: initializer,
        parameters: vec![],
        span,
    });
    class.methods.push(MirMethodDeclaration {
        id: method,
        name: "exhaust_aliases".to_owned(),
        kind: MirMethodKind::instance(MirReceiverAccess::ReadOnly),
        parameters: parameters.clone(),
        return_type: MirType::Unit,
        span,
    });

    let callable = CallableId::Method(method);
    let receiver = StorageId::new(callable, 0);
    let mut storage = vec![receiver_storage(callable, receiver, ids.container, span)];
    for (index, parameter) in parameters.iter().copied().enumerate() {
        let id = StorageId::new(callable, index + 1);
        storage.push(match parameter.mode {
            MirParameterMode::Value => {
                value_parameter_storage(callable, id, index, parameter.ty, span)
            }
            MirParameterMode::ReadOnlyAlias => alias_storage(
                callable,
                id,
                index,
                ids.container,
                MirAliasAccess::ReadOnly,
                span,
            ),
            MirParameterMode::MutableAlias => alias_storage(
                callable,
                id,
                index,
                ids.container,
                MirAliasAccess::Mutable,
                span,
            ),
        });
    }
    program.member_definitions = MirMemberDefinitionTable::new(vec![
        super::object_fixtures::empty_initializer_definition(initializer, span),
        member_definition(callable, receiver, storage, vec![], vec![], span),
    ]);

    let main = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    main.body.blocks[0].instructions.insert(
        0,
        MirInstruction::Initialize(MirInitialize {
            destination: MirPlace::base(ids.first),
            target: initializer,
            arguments: vec![],
            span,
        }),
    );
    let first_value = main.values.len();
    let mut arguments = Vec::with_capacity(parameters.len());
    let mut float_index = 0;
    for parameter in parameters {
        match parameter.mode {
            MirParameterMode::ReadOnlyAlias | MirParameterMode::MutableAlias => {
                arguments.push(MirArgument::Place(MirPlace::base(ids.first)));
            }
            MirParameterMode::Value => {
                let value = ValueId::new(main.function, first_value + float_index);
                float_index += 1;
                main.values.push(MirValue {
                    id: value,
                    ty: MirType::F64,
                    span,
                });
                main.body.blocks[0].instructions.push(assignment(
                    main.function,
                    value.index(),
                    MirRvalueKind::ConstantF64Bits((float_index as f64).to_bits()),
                    MirType::F64,
                    span,
                ));
                arguments.push(MirArgument::Value(value));
            }
        }
    }
    main.body.blocks[0]
        .instructions
        .push(MirInstruction::Call(MirCall {
            target: MirCallTarget::Method(MirMethodCallTarget::Direct(method)),
            receiver: Some(
                MirMethodReceiver::exact(
                    MirPlace::base(ids.first),
                    ids.container,
                    MirAliasAccess::Mutable,
                )
                .into(),
            ),
            arguments,
            result: None,
            shared_result: None,
            destination: None,
            span,
        }));
    main.body.blocks[0]
        .instructions
        .push(MirInstruction::Cleanup(MirCleanup {
            destination: ids.first.into(),
            target: ids.container,
            span,
        }));
    fixture_add_body_storage_lifetimes(&main.storage, &mut main.body, span);
    program
}

fn unit_declaration(
    id: FunctionId,
    name: &str,
    parameters: Vec<MirParameter>,
    span: crate::source::Span,
) -> MirFunctionDeclaration {
    fixture_function_declaration(
        id,
        name,
        parameters,
        MirType::Unit,
        MirFunctionLinkage::Internal,
        span,
    )
}

fn alias_add_definition(
    id: FunctionId,
    class: ClassId,
    value_field: FieldId,
    sum: FunctionId,
    span: crate::source::Span,
) -> MirFunctionDefinition {
    let alias = StorageId::new(id, 0);
    let amount = StorageId::new(id, 1);
    let values = values(id.into(), &[MirType::I64, MirType::I64, MirType::I64], span);
    let alias_value = MirPlace::alias_parameter(alias).project_field(value_field);
    function_definition(
        id,
        vec![
            alias_storage(id.into(), alias, 0, class, MirAliasAccess::Mutable, span),
            value_parameter_storage(id.into(), amount, 1, MirType::I64, span),
        ],
        values,
        vec![
            assign(
                ValueId::new(id, 0),
                MirRvalueKind::Load(alias_value.clone()),
                MirType::I64,
                span,
            ),
            assign(
                ValueId::new(id, 1),
                MirRvalueKind::Load(MirPlace::base(amount)),
                MirType::I64,
                span,
            ),
            MirInstruction::Call(MirCall {
                target: MirCallTarget::Direct(sum),
                receiver: None,
                arguments: MirArgument::values([ValueId::new(id, 0), ValueId::new(id, 1)]),
                result: Some(ValueId::new(id, 2)),
                shared_result: None,
                destination: None,
                span,
            }),
            store(alias_value, ValueId::new(id, 2), span),
        ],
        span,
    )
}

fn alias_forward_definition(
    id: FunctionId,
    class: ClassId,
    add: FunctionId,
    span: crate::source::Span,
) -> MirFunctionDefinition {
    let first = StorageId::new(id, 0);
    let second = StorageId::new(id, 1);
    let amount = StorageId::new(id, 2);
    function_definition(
        id,
        vec![
            alias_storage(id.into(), first, 0, class, MirAliasAccess::Mutable, span),
            alias_storage(id.into(), second, 1, class, MirAliasAccess::Mutable, span),
            value_parameter_storage(id.into(), amount, 2, MirType::I64, span),
        ],
        values(id.into(), &[MirType::I64], span),
        vec![
            assign(
                ValueId::new(id, 0),
                MirRvalueKind::Load(MirPlace::base(amount)),
                MirType::I64,
                span,
            ),
            alias_call(add, first, ValueId::new(id, 0), span),
            alias_call(add, second, ValueId::new(id, 0), span),
        ],
        span,
    )
}

fn alias_initializer_definition(
    id: InitializerId,
    value_field: FieldId,
    span: crate::source::Span,
) -> MirMemberDefinition {
    let callable = CallableId::Initializer(id);
    let receiver = StorageId::new(callable, 0);
    let source = StorageId::new(callable, 1);
    let result = ValueId::new(callable, 0);
    member_definition(
        callable,
        receiver,
        vec![
            receiver_storage(callable, receiver, id.class(), span),
            alias_storage(
                callable,
                source,
                0,
                id.class(),
                MirAliasAccess::ReadOnly,
                span,
            ),
        ],
        vec![MirValue {
            id: result,
            ty: MirType::I64,
            span,
        }],
        vec![
            assign(
                result,
                MirRvalueKind::Load(MirPlace::alias_parameter(source).project_field(value_field)),
                MirType::I64,
                span,
            ),
            store(
                MirPlace::base(receiver).project_field(value_field),
                result,
                span,
            ),
        ],
        span,
    )
}

fn alias_method_definition(
    id: MethodId,
    class: ClassId,
    add: FunctionId,
    span: crate::source::Span,
) -> MirMemberDefinition {
    let callable = CallableId::Method(id);
    let receiver = StorageId::new(callable, 0);
    let alias = StorageId::new(callable, 1);
    let float = StorageId::new(callable, 2);
    let amount = StorageId::new(callable, 3);
    member_definition(
        callable,
        receiver,
        vec![
            receiver_storage(callable, receiver, class, span),
            alias_storage(callable, alias, 0, class, MirAliasAccess::Mutable, span),
            value_parameter_storage(callable, float, 1, MirType::F64, span),
            value_parameter_storage(callable, amount, 2, MirType::I64, span),
        ],
        values(callable, &[MirType::I64], span),
        vec![
            assign(
                ValueId::new(callable, 0),
                MirRvalueKind::Load(MirPlace::base(amount)),
                MirType::I64,
                span,
            ),
            alias_call(add, alias, ValueId::new(callable, 0), span),
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
    let parameters = storage.iter().map(|storage| storage.id).collect();
    fixture_function_definition(
        id,
        OneBlockDefinition {
            return_storage: None,
            parameters,
            storage,
            values,
            instructions,
            terminator: Some(MirTerminator::Return { value: None, span }),
            span,
        },
    )
}

fn member_definition(
    callable: CallableId,
    receiver: StorageId,
    storage: Vec<MirStorage>,
    values: Vec<MirValue>,
    instructions: Vec<MirInstruction>,
    span: crate::source::Span,
) -> MirMemberDefinition {
    let parameters = storage.iter().skip(1).map(|storage| storage.id).collect();
    fixture_member_definition(
        callable,
        Some(receiver),
        OneBlockDefinition {
            return_storage: None,
            parameters,
            storage,
            values,
            instructions,
            terminator: Some(MirTerminator::Return { value: None, span }),
            span,
        },
    )
}

fn alias_storage(
    callable: CallableId,
    id: StorageId,
    index: usize,
    class: ClassId,
    access: MirAliasAccess,
    span: crate::source::Span,
) -> MirStorage {
    fixture_storage(
        id,
        Some(BindingId::Parameter(ParameterId::new(callable, index))),
        format!("alias{index}"),
        MirStorageKind::AliasParameter(access),
        MirType::Class(class),
        span,
    )
}

fn value_parameter_storage(
    callable: CallableId,
    id: StorageId,
    index: usize,
    ty: MirType,
    span: crate::source::Span,
) -> MirStorage {
    fixture_storage(
        id,
        Some(BindingId::Parameter(ParameterId::new(callable, index))),
        format!("value{index}"),
        MirStorageKind::Parameter,
        ty,
        span,
    )
}

fn receiver_storage(
    callable: CallableId,
    id: StorageId,
    class: ClassId,
    span: crate::source::Span,
) -> MirStorage {
    assert_eq!(id.callable(), callable);
    fixture_receiver_storage(id, class, span)
}

fn values(callable: CallableId, types: &[MirType], span: crate::source::Span) -> Vec<MirValue> {
    types
        .iter()
        .copied()
        .enumerate()
        .map(|(index, ty)| fixture_value(ValueId::new(callable, index), ty, span))
        .collect()
}

fn alias_call(
    target: FunctionId,
    alias: StorageId,
    amount: ValueId,
    span: crate::source::Span,
) -> MirInstruction {
    fixture_call(
        MirCallTarget::Direct(target),
        None,
        vec![
            MirArgument::Place(MirPlace::alias_parameter(alias)),
            MirArgument::Value(amount),
        ],
        None,
        None,
        span,
    )
}

fn assign(
    result: ValueId,
    kind: MirRvalueKind,
    ty: MirType,
    span: crate::source::Span,
) -> MirInstruction {
    fixture_assign(result, kind, ty, span)
}
