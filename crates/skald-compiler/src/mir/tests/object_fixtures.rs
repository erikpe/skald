use super::*;

pub(super) struct ObjectFixtureIds {
    pub outer: ClassId,
    pub inner_value: FieldId,
    pub outer_inner: FieldId,
    pub object_storage: StorageId,
}

pub(super) fn object_mir() -> (MirProgram, ObjectFixtureIds) {
    let mut program = lower_text("fn main() -> i64 { return 7; }");
    let function_id = program.entry_function;
    let function = program.definitions.get_mut_for_test(function_id).unwrap();
    let span = function.span;

    let inner = ClassId::new(0);
    let outer = ClassId::new(1);
    let inner_value = FieldId::new(inner, 0);
    let outer_inner = FieldId::new(outer, 0);
    let outer_initializer = InitializerId::new(outer, 0);
    let outer_method = MethodId::new(outer, 0);

    program.classes = MirClassDeclarationTable::new(vec![
        MirClassDeclaration {
            id: inner,
            module: crate::identity::ModuleId::new(0),
            name: "Inner".to_owned(),
            direct_base: None,
            conformances: vec![],
            static_fields: vec![],
            fields: vec![MirFieldDeclaration {
                id: inner_value,
                name: "value".to_owned(),
                ty: MirType::I64,
                span,
            }],
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
            id: outer,
            module: crate::identity::ModuleId::new(0),
            name: "Outer".to_owned(),
            direct_base: None,
            conformances: vec![],
            static_fields: vec![],
            fields: vec![MirFieldDeclaration {
                id: outer_inner,
                name: "inner".to_owned(),
                ty: MirType::Class(inner),
                span,
            }],
            initializers: vec![MirInitializerDeclaration {
                id: outer_initializer,
                parameters: MirParameter::values([MirType::I64]),
                span,
            }],
            copy_constructor_declaration: None,
            copy_constructor: MirCopyCapability::Unavailable,
            copy_assignment_declaration: None,
            copy_assignment: MirCopyCapability::Unavailable,
            destruction: MirDestructionPlan::new(None, &[outer_inner]),
            methods: vec![MirMethodDeclaration {
                id: outer_method,
                name: "get".to_owned(),
                kind: MirMethodKind::instance(MirReceiverAccess::ReadOnly),
                parameters: vec![],
                return_type: MirType::I64,
                span,
            }],
            span,
        },
    ]);
    program.member_definitions =
        MirMemberDefinitionTable::new(vec![fixture_empty_member_definition(
            outer_initializer.into(),
            outer,
            &[MirParameter::value(MirType::I64)],
            span,
        )]);

    let object_storage = StorageId::new(function_id, 0);
    function.storage.push(MirStorage {
        id: object_storage,
        source: Some(BindingId::Local(LocalId::new(function_id, 0))),
        name: "object".to_owned(),
        kind: MirStorageKind::Local,
        ty: MirType::Class(outer),
        span,
    });
    let loaded = ValueId::new(function_id, 1);
    let method_result = ValueId::new(function_id, 2);
    function.values.extend([
        MirValue {
            id: loaded,
            ty: MirType::I64,
            span,
        },
        MirValue {
            id: method_result,
            ty: MirType::I64,
            span,
        },
    ]);
    let block = &mut function.body.blocks[0];
    block
        .instructions
        .push(MirInstruction::Initialize(MirInitialize {
            destination: object_storage.into(),
            target: outer_initializer,
            arguments: MirArgument::values([ValueId::new(function_id, 0)]),
            span,
        }));
    block
        .instructions
        .push(MirInstruction::Assign(MirAssignment {
            result: loaded,
            rvalue: MirRvalue {
                kind: MirRvalueKind::Load(
                    MirPlace::base(object_storage)
                        .project_field(outer_inner)
                        .project_field(inner_value),
                ),
                ty: MirType::I64,
            },
            span,
        }));
    block.instructions.push(MirInstruction::Call(MirCall {
        target: MirCallTarget::Method(MirMethodCallTarget::Direct(outer_method)),
        receiver: Some(MirMethodReceiver::exact(object_storage.into(), outer).into()),
        arguments: vec![],
        result: Some(method_result),
        shared_result: None,
        destination: None,
        span,
    }));
    block.instructions.push(MirInstruction::Cleanup(MirCleanup {
        destination: object_storage.into(),
        target: outer,
        span,
    }));
    fixture_add_body_storage_lifetimes(&function.storage, &mut function.body, span);

    (
        program,
        ObjectFixtureIds {
            outer,
            inner_value,
            outer_inner,
            object_storage,
        },
    )
}

pub(super) fn messages(program: &MirProgram) -> Vec<String> {
    verify_mir(program)
        .unwrap_err()
        .iter()
        .map(|error| error.message.clone())
        .collect()
}
