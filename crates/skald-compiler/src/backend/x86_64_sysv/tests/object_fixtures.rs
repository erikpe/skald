use super::*;
use crate::identity::CallableId;

pub(super) struct ObjectProgramIds {
    pub nested: ClassId,
    pub container: ClassId,
    pub nested_small: FieldId,
    pub nested_payload: FieldId,
    pub container_tag: FieldId,
    pub container_nested: FieldId,
    pub first: StorageId,
    pub empty: StorageId,
    pub second: StorageId,
}

pub(super) fn projected_object_program() -> (MirProgram, ObjectProgramIds) {
    let mut program = lower_source_to_mir("fn main() -> i64 { return 0; }");
    let function_id = program.entry_function;
    let function = program.definitions.get_mut_for_test(function_id).unwrap();
    let span = function.span;

    let nested = ClassId::new(0);
    let container = ClassId::new(1);
    let empty_class = ClassId::new(2);
    let nested_small = FieldId::new(nested, 0);
    let nested_payload = FieldId::new(nested, 1);
    let container_tag = FieldId::new(container, 0);
    let container_nested = FieldId::new(container, 1);
    let container_tail = FieldId::new(container, 2);
    program.classes = MirClassDeclarationTable::new(vec![
        MirClassDeclaration {
            id: nested,
            module: crate::identity::ModuleId::new(0),
            name: "Nested".to_owned(),
            direct_base: None,
            conformances: vec![],
            fields: vec![
                field(nested_small, "small", MirType::U8, span),
                field(nested_payload, "payload", MirType::F64, span),
            ],
            initializers: vec![],
            copy_constructor_declaration: None,
            copy_constructor: MirCopyCapability::Unavailable,
            copy_assignment_declaration: None,
            copy_assignment: MirCopyCapability::Unavailable,
            destruction: MirDestructionPlan::new(None, &[]),
            methods: vec![],
            span,
        },
        MirClassDeclaration {
            id: container,
            module: crate::identity::ModuleId::new(0),
            name: "Container".to_owned(),
            direct_base: None,
            conformances: vec![],
            fields: vec![
                field(container_tag, "tag", MirType::Bool, span),
                field(container_nested, "nested", MirType::Class(nested), span),
                field(container_tail, "tail", MirType::U8, span),
            ],
            initializers: vec![],
            copy_constructor_declaration: None,
            copy_constructor: MirCopyCapability::Unavailable,
            copy_assignment_declaration: None,
            copy_assignment: MirCopyCapability::Unavailable,
            destruction: MirDestructionPlan::new(None, &[container_nested]),
            methods: vec![],
            span,
        },
        MirClassDeclaration {
            id: empty_class,
            module: crate::identity::ModuleId::new(0),
            name: "Empty".to_owned(),
            direct_base: None,
            conformances: vec![],
            fields: vec![],
            initializers: vec![],
            copy_constructor_declaration: None,
            copy_constructor: MirCopyCapability::Unavailable,
            copy_assignment_declaration: None,
            copy_assignment: MirCopyCapability::Unavailable,
            destruction: MirDestructionPlan::new(None, &[]),
            methods: vec![],
            span,
        },
    ]);

    let first = StorageId::new(function_id, 0);
    let empty = StorageId::new(function_id, 1);
    let second = StorageId::new(function_id, 2);
    for (index, (id, name, ty)) in [
        (first, "first", MirType::Class(container)),
        (empty, "empty", MirType::Class(empty_class)),
        (second, "second", MirType::Class(container)),
    ]
    .into_iter()
    .enumerate()
    {
        function.storage.push(MirStorage {
            id,
            source: Some(BindingId::Local(LocalId::new(function_id, index))),
            name: name.to_owned(),
            kind: MirStorageKind::Local,
            ty,
            span,
        });
    }

    let value_types = [
        MirType::U8,
        MirType::U8,
        MirType::F64,
        MirType::F64,
        MirType::Bool,
        MirType::Bool,
    ];
    function.values.extend(
        value_types
            .into_iter()
            .enumerate()
            .map(|(index, ty)| MirValue {
                id: ValueId::new(function_id, index + 1),
                ty,
                span,
            }),
    );

    let small_place = MirPlace::base(first)
        .project_field(container_nested)
        .project_field(nested_small);
    let payload_place = MirPlace::base(first)
        .project_field(container_nested)
        .project_field(nested_payload);
    let tag_place = MirPlace::base(first).project_field(container_tag);
    function.body.blocks[0].instructions.extend([
        assignment(
            function_id,
            1,
            MirRvalueKind::ConstantU8(255),
            MirType::U8,
            span,
        ),
        store(small_place.clone(), ValueId::new(function_id, 1), span),
        assignment(
            function_id,
            2,
            MirRvalueKind::Load(small_place),
            MirType::U8,
            span,
        ),
        assignment(
            function_id,
            3,
            MirRvalueKind::ConstantF64Bits(1.5_f64.to_bits()),
            MirType::F64,
            span,
        ),
        store(payload_place.clone(), ValueId::new(function_id, 3), span),
        assignment(
            function_id,
            4,
            MirRvalueKind::Load(payload_place),
            MirType::F64,
            span,
        ),
        assignment(
            function_id,
            5,
            MirRvalueKind::ConstantBool(true),
            MirType::Bool,
            span,
        ),
        store(tag_place.clone(), ValueId::new(function_id, 5), span),
        assignment(
            function_id,
            6,
            MirRvalueKind::Load(tag_place),
            MirType::Bool,
            span,
        ),
    ]);
    fixture_add_body_storage_lifetimes(&function.storage, &mut function.body, span);

    (
        program,
        ObjectProgramIds {
            nested,
            container,
            nested_small,
            nested_payload,
            container_tag,
            container_nested,
            first,
            empty,
            second,
        },
    )
}

pub(super) fn counter_member_program() -> MirProgram {
    let mut program = lower_source_to_mir(concat!(
        "extern fn ska_rt_println_i64(value: i64) -> unit;\n",
        "fn sum(a: i64, b: i64) -> i64 { return a + b; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    let span = program.span;
    let class = ClassId::new(0);
    let value_field = FieldId::new(class, 0);
    let initializer = InitializerId::new(class, 0);
    let add = MethodId::new(class, 0);
    let get = MethodId::new(class, 1);
    let get_via_receiver = MethodId::new(class, 2);
    program.classes = MirClassDeclarationTable::new(vec![MirClassDeclaration {
        id: class,
        module: crate::identity::ModuleId::new(0),
        name: "Counter".to_owned(),
        direct_base: None,
        conformances: vec![],
        fields: vec![field(value_field, "value", MirType::I64, span)],
        initializers: vec![MirInitializerDeclaration {
            id: initializer,
            parameters: MirParameter::values([MirType::I64]),
            span,
        }],
        copy_constructor_declaration: None,
        copy_constructor: MirCopyCapability::Unavailable,
        copy_assignment_declaration: None,
        copy_assignment: MirCopyCapability::Unavailable,
        destruction: MirDestructionPlan::new(None, &[]),
        methods: vec![
            MirMethodDeclaration {
                id: add,
                name: "add".to_owned(),
                kind: MirMethodKind::instance(MirReceiverAccess::Mutable),
                parameters: MirParameter::values([MirType::I64]),
                return_type: MirType::Unit,
                span,
            },
            MirMethodDeclaration {
                id: get,
                name: "get".to_owned(),
                kind: MirMethodKind::instance(MirReceiverAccess::ReadOnly),
                parameters: vec![],
                return_type: MirType::I64,
                span,
            },
            MirMethodDeclaration {
                id: get_via_receiver,
                name: "get_via_receiver".to_owned(),
                kind: MirMethodKind::instance(MirReceiverAccess::ReadOnly),
                parameters: vec![],
                return_type: MirType::I64,
                span,
            },
        ],
        span,
    }]);
    program.member_definitions = MirMemberDefinitionTable::new(vec![
        initializer_definition(initializer, value_field, span),
        add_definition(add, value_field, FunctionId::new(1), span),
        get_definition(get, value_field, span),
        forwarding_get_definition(get_via_receiver, get, span),
    ]);

    let main_id = program.entry_function;
    let main = program.definitions.get_mut_for_test(main_id).unwrap();
    let object = StorageId::new(main_id, 0);
    main.storage.push(MirStorage {
        id: object,
        source: Some(BindingId::Local(LocalId::new(main_id, 0))),
        name: "counter".to_owned(),
        kind: MirStorageKind::Local,
        ty: MirType::Class(class),
        span,
    });
    main.values.extend([
        MirValue {
            id: ValueId::new(main_id, 1),
            ty: MirType::I64,
            span,
        },
        MirValue {
            id: ValueId::new(main_id, 2),
            ty: MirType::I64,
            span,
        },
        MirValue {
            id: ValueId::new(main_id, 3),
            ty: MirType::I64,
            span,
        },
    ]);
    main.body.blocks[0].instructions.extend([
        assignment(
            main_id,
            1,
            MirRvalueKind::ConstantI64(40),
            MirType::I64,
            span,
        ),
        MirInstruction::Initialize(MirInitialize {
            destination: object.into(),
            target: initializer,
            arguments: MirArgument::values([ValueId::new(main_id, 1)]),
            span,
        }),
        assignment(
            main_id,
            2,
            MirRvalueKind::ConstantI64(2),
            MirType::I64,
            span,
        ),
        MirInstruction::Call(MirCall {
            target: MirCallTarget::Method(MirMethodCallTarget::Direct(add)),
            receiver: Some(MirMethodReceiver::exact(object.into(), class).into()),
            arguments: MirArgument::values([ValueId::new(main_id, 2)]),
            result: None,
            shared_result: None,
            destination: None,
            span,
        }),
        MirInstruction::Call(MirCall {
            target: MirCallTarget::Method(MirMethodCallTarget::Direct(get_via_receiver)),
            receiver: Some(MirMethodReceiver::exact(object.into(), class).into()),
            arguments: vec![],
            result: Some(ValueId::new(main_id, 3)),
            shared_result: None,
            destination: None,
            span,
        }),
        MirInstruction::Call(MirCall {
            target: MirCallTarget::Direct(FunctionId::new(0)),
            receiver: None,
            arguments: MirArgument::values([ValueId::new(main_id, 3)]),
            result: None,
            shared_result: None,
            destination: None,
            span,
        }),
        MirInstruction::Cleanup(MirCleanup {
            destination: object.into(),
            target: class,
            span,
        }),
    ]);
    fixture_add_body_storage_lifetimes(&main.storage, &mut main.body, span);
    program
}

pub(super) fn exhausted_receiver_abi_program() -> MirProgram {
    let (mut program, ids) = projected_object_program();
    let initializer = InitializerId::new(ids.container, 0);
    let method = MethodId::new(ids.container, 0);
    let mut parameter_types = vec![MirType::I64; 5];
    parameter_types.extend([MirType::F64; 8]);
    parameter_types.extend([MirType::I64, MirType::F64]);
    let class = &mut program.classes.entries_mut_for_test()[ids.container.index()];
    class.initializers.push(MirInitializerDeclaration {
        id: initializer,
        parameters: vec![],
        span: program.span,
    });
    class.methods.push(MirMethodDeclaration {
        id: method,
        name: "exhaust".to_owned(),
        kind: MirMethodKind::instance(MirReceiverAccess::ReadOnly),
        parameters: MirParameter::values(parameter_types.clone()),
        return_type: MirType::Unit,
        span: program.span,
    });

    let callable = method.into();
    let receiver = StorageId::new(callable, 0);
    let parameters: Vec<_> = parameter_types
        .iter()
        .enumerate()
        .map(|(index, ty)| {
            parameter_storage(
                callable,
                StorageId::new(callable, index + 1),
                index,
                *ty,
                program.span,
            )
        })
        .collect();
    program.member_definitions = MirMemberDefinitionTable::new(vec![
        empty_initializer_definition(initializer, program.span),
        MirMemberDefinition {
            callable,
            class_owner: ids.container,
            return_storage: None,
            receiver: Some(receiver),
            parameters: parameters.iter().map(|parameter| parameter.id).collect(),
            storage: std::iter::once(receiver_storage(
                callable,
                receiver,
                ids.container,
                program.span,
            ))
            .chain(parameters)
            .collect(),
            values: vec![],
            body: MirBody {
                entry: BlockId::new(callable, 0),
                blocks: vec![MirBasicBlock {
                    id: BlockId::new(callable, 0),
                    instructions: vec![],
                    terminator: Some(MirTerminator::Return {
                        value: None,
                        span: program.span,
                    }),
                    span: program.span,
                }],
            },
            span: program.span,
        },
    ]);

    let function = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    function.body.blocks[0].instructions.insert(
        0,
        MirInstruction::Initialize(MirInitialize {
            destination: MirPlace::base(ids.first),
            target: initializer,
            arguments: vec![],
            span: program.span,
        }),
    );
    let first_value = function.values.len();
    let mut arguments = Vec::with_capacity(parameter_types.len());
    for (index, ty) in parameter_types.into_iter().enumerate() {
        let value = ValueId::new(function.function, first_value + index);
        function.values.push(MirValue {
            id: value,
            ty,
            span: program.span,
        });
        let kind = if ty == MirType::F64 {
            MirRvalueKind::ConstantF64Bits((index as f64).to_bits())
        } else {
            MirRvalueKind::ConstantI64(index as i64)
        };
        function.body.blocks[0]
            .instructions
            .push(MirInstruction::Assign(MirAssignment {
                result: value,
                rvalue: MirRvalue { kind, ty },
                span: program.span,
            }));
        arguments.push(MirArgument::Value(value));
    }
    function.body.blocks[0]
        .instructions
        .push(MirInstruction::Call(MirCall {
            target: MirCallTarget::Method(MirMethodCallTarget::Direct(method)),
            receiver: Some(MirMethodReceiver::exact(ids.first.into(), ids.container).into()),
            arguments,
            result: None,
            shared_result: None,
            destination: None,
            span: program.span,
        }));
    function.body.blocks[0]
        .instructions
        .push(MirInstruction::Cleanup(MirCleanup {
            destination: ids.first.into(),
            target: ids.container,
            span: program.span,
        }));
    fixture_add_body_storage_lifetimes(&function.storage, &mut function.body, program.span);
    program
}

pub(super) fn empty_initializer_definition(
    id: InitializerId,
    span: crate::source::Span,
) -> MirMemberDefinition {
    let callable = CallableId::Initializer(id);
    let receiver = StorageId::new(callable, 0);
    fixture_member_definition(
        callable,
        Some(receiver),
        OneBlockDefinition {
            return_storage: None,
            parameters: vec![],
            storage: vec![receiver_storage(callable, receiver, id.class(), span)],
            values: vec![],
            instructions: vec![],
            terminator: Some(MirTerminator::Return { value: None, span }),
            span,
        },
    )
}

fn initializer_definition(
    id: InitializerId,
    value_field: FieldId,
    span: crate::source::Span,
) -> MirMemberDefinition {
    let callable = id.into();
    let receiver = StorageId::new(callable, 0);
    let parameter = StorageId::new(callable, 1);
    let value = ValueId::new(callable, 0);
    fixture_member_definition(
        callable,
        Some(receiver),
        OneBlockDefinition {
            return_storage: None,
            parameters: vec![parameter],
            storage: vec![
                receiver_storage(callable, receiver, id.class(), span),
                parameter_storage(callable, parameter, 0, MirType::I64, span),
            ],
            values: vec![fixture_value(value, MirType::I64, span)],
            instructions: vec![
                fixture_assign(
                    value,
                    MirRvalueKind::Load(parameter.into()),
                    MirType::I64,
                    span,
                ),
                store(
                    MirPlace::base(receiver).project_field(value_field),
                    value,
                    span,
                ),
            ],
            terminator: Some(MirTerminator::Return { value: None, span }),
            span,
        },
    )
}

fn add_definition(
    id: MethodId,
    value_field: FieldId,
    sum: FunctionId,
    span: crate::source::Span,
) -> MirMemberDefinition {
    let callable = id.into();
    let receiver = StorageId::new(callable, 0);
    let parameter = StorageId::new(callable, 1);
    let values: Vec<_> = (0..3)
        .map(|index| fixture_value(ValueId::new(callable, index), MirType::I64, span))
        .collect();
    let receiver_value = MirPlace::base(receiver).project_field(value_field);
    fixture_member_definition(
        callable,
        Some(receiver),
        OneBlockDefinition {
            return_storage: None,
            parameters: vec![parameter],
            storage: vec![
                receiver_storage(callable, receiver, id.class(), span),
                parameter_storage(callable, parameter, 0, MirType::I64, span),
            ],
            values,
            instructions: vec![
                fixture_assign(
                    ValueId::new(callable, 0),
                    MirRvalueKind::Load(receiver_value.clone()),
                    MirType::I64,
                    span,
                ),
                fixture_assign(
                    ValueId::new(callable, 1),
                    MirRvalueKind::Load(parameter.into()),
                    MirType::I64,
                    span,
                ),
                fixture_call(
                    MirCallTarget::Direct(sum),
                    None,
                    MirArgument::values([ValueId::new(callable, 0), ValueId::new(callable, 1)]),
                    Some(ValueId::new(callable, 2)),
                    None,
                    span,
                ),
                store(receiver_value, ValueId::new(callable, 2), span),
            ],
            terminator: Some(MirTerminator::Return { value: None, span }),
            span,
        },
    )
}

fn get_definition(
    id: MethodId,
    value_field: FieldId,
    span: crate::source::Span,
) -> MirMemberDefinition {
    let callable = id.into();
    let receiver = StorageId::new(callable, 0);
    let result = ValueId::new(callable, 0);
    fixture_member_definition(
        callable,
        Some(receiver),
        OneBlockDefinition {
            return_storage: None,
            parameters: vec![],
            storage: vec![receiver_storage(callable, receiver, id.class(), span)],
            values: vec![fixture_value(result, MirType::I64, span)],
            instructions: vec![fixture_assign(
                result,
                MirRvalueKind::Load(MirPlace::base(receiver).project_field(value_field)),
                MirType::I64,
                span,
            )],
            terminator: Some(MirTerminator::Return {
                value: Some(result),
                span,
            }),
            span,
        },
    )
}

fn forwarding_get_definition(
    id: MethodId,
    target: MethodId,
    span: crate::source::Span,
) -> MirMemberDefinition {
    let callable = id.into();
    let receiver = StorageId::new(callable, 0);
    let result = ValueId::new(callable, 0);
    fixture_member_definition(
        callable,
        Some(receiver),
        OneBlockDefinition {
            return_storage: None,
            parameters: vec![],
            storage: vec![receiver_storage(callable, receiver, id.class(), span)],
            values: vec![fixture_value(result, MirType::I64, span)],
            instructions: vec![fixture_call(
                MirCallTarget::Method(MirMethodCallTarget::Direct(target)),
                Some(MirMethodReceiver {
                    place: receiver.into(),
                    origin: Box::new(MirObjectOrigin::Forwarded {
                        carrier: receiver,
                        static_target: MirViewTarget::Class(id.class()),
                        access: MirAliasAccess::ReadOnly,
                        dispatch_limit: None,
                        span,
                    }),
                }),
                vec![],
                Some(result),
                None,
                span,
            )],
            terminator: Some(MirTerminator::Return {
                value: Some(result),
                span,
            }),
            span,
        },
    )
}

fn receiver_storage(
    callable: crate::identity::CallableId,
    id: StorageId,
    class: ClassId,
    span: crate::source::Span,
) -> MirStorage {
    assert_eq!(id.callable(), callable);
    fixture_receiver_storage(id, class, span)
}

fn parameter_storage(
    callable: crate::identity::CallableId,
    id: StorageId,
    index: usize,
    ty: MirType,
    span: crate::source::Span,
) -> MirStorage {
    fixture_storage(
        id,
        Some(BindingId::Parameter(ParameterId::new(callable, index))),
        format!("p{index}"),
        MirStorageKind::Parameter,
        ty,
        span,
    )
}

pub(super) fn field(
    id: FieldId,
    name: &str,
    ty: MirType,
    span: crate::source::Span,
) -> MirFieldDeclaration {
    MirFieldDeclaration {
        id,
        name: name.to_owned(),
        ty,
        span,
    }
}

pub(super) fn assignment(
    function: FunctionId,
    index: usize,
    kind: MirRvalueKind,
    ty: MirType,
    span: crate::source::Span,
) -> MirInstruction {
    fixture_assign(ValueId::new(function, index), kind, ty, span)
}

pub(super) fn store(place: MirPlace, value: ValueId, span: crate::source::Span) -> MirInstruction {
    fixture_store(place, value, span)
}
