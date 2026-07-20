use super::*;

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
            name: "Nested".to_owned(),
            fields: vec![
                field(nested_small, "small", MirType::U8, span),
                field(nested_payload, "payload", MirType::F64, span),
            ],
            initializers: vec![],
            methods: vec![],
            span,
        },
        MirClassDeclaration {
            id: container,
            name: "Container".to_owned(),
            fields: vec![
                field(container_tag, "tag", MirType::Bool, span),
                field(container_nested, "nested", MirType::Class(nested), span),
                field(container_tail, "tail", MirType::U8, span),
            ],
            initializers: vec![],
            methods: vec![],
            span,
        },
        MirClassDeclaration {
            id: empty_class,
            name: "Empty".to_owned(),
            fields: vec![],
            initializers: vec![],
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
            source: BindingId::Local(LocalId::new(function_id, index)),
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

fn field(id: FieldId, name: &str, ty: MirType, span: crate::source::Span) -> MirFieldDeclaration {
    MirFieldDeclaration {
        id,
        name: name.to_owned(),
        ty,
        span,
    }
}

fn assignment(
    function: FunctionId,
    index: usize,
    kind: MirRvalueKind,
    ty: MirType,
    span: crate::source::Span,
) -> MirInstruction {
    MirInstruction::Assign(MirAssignment {
        result: ValueId::new(function, index),
        rvalue: MirRvalue { kind, ty },
        span,
    })
}

fn store(place: MirPlace, value: ValueId, span: crate::source::Span) -> MirInstruction {
    MirInstruction::Store(MirStore {
        destination: place,
        value,
        span,
    })
}
