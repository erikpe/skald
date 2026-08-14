use super::{object_fixtures::*, *};

#[test]
fn verifies_class_metadata_nested_places_initialization_and_receiver_calls() {
    let (program, _) = object_mir();
    assert!(verify_mir(&program).is_ok());
}

#[test]
fn lowers_complete_source_ordered_initializer_vectors_from_hir() {
    let checked = type_check_source(concat!(
        "class Empty { private init() {} }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let mut hir = checked.hir.unwrap();
    let class = ClassId::new(0);
    let second = InitializerId::new(class, 1);

    let declaration = &mut hir.classes.entries_mut_for_test()[0];
    let mut second_declaration = declaration.initializers[0].clone();
    second_declaration.id = second;
    declaration.initializers.push(second_declaration);

    let definition = &mut hir.class_definitions.entries_mut_for_test()[0];
    let mut second_definition = definition.initializers[0].clone();
    second_definition.callable = second.into();
    definition.initializers.push(second_definition);

    let mir = lower_hir(&hir);
    assert_eq!(
        mir.class(class)
            .unwrap()
            .initializers
            .iter()
            .map(|initializer| initializer.id)
            .collect::<Vec<_>>(),
        [InitializerId::new(class, 0), InitializerId::new(class, 1)]
    );
    assert!(mir.member_definition(second.into()).is_some());
    assert!(verify_mir(&mir).is_ok());
    let dump = dump_mir(&mir);
    assert!(dump.contains("Initializer c0:init0()"));
    assert!(dump.contains("Initializer c0:init1()"));
    assert!(dump.contains("MemberDefinition c0:init1"));
}

#[test]
fn verifier_rejects_initializer_density_and_missing_bodies() {
    let mut wrong_density = lower_text(concat!(
        "class Empty { init() {} }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    wrong_density.classes.entries_mut_for_test()[0].initializers[0].id =
        InitializerId::new(ClassId::new(0), 1);
    let errors = verify_mir(&wrong_density).unwrap_err().to_string();
    assert!(errors.contains("initializer table index 0 contains c0:init1"));

    let mut missing_body = lower_text(concat!(
        "class Empty { init() {} }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    let initializer = InitializerId::new(ClassId::new(0), 0);
    missing_body
        .member_definitions
        .remove_for_test(initializer.into());
    let errors = verify_mir(&missing_body).unwrap_err().to_string();
    assert!(errors.contains("initializer c0:init0 has no member definition"));
}

#[test]
fn dumps_object_metadata_and_projected_places_deterministically() {
    let (program, _) = object_mir();
    let dump = dump_mir(&program);

    assert_eq!(
        dump,
        concat!(
            "MirProgram @0..30\n",
            "  SelectedModule m0\n",
            "  Modules\n",
            "    Module m0 main source 0 provider provider0 package package0\n",
            "  Entry f0\n",
            "  Classes\n",
            "    Class c0 module m0 \"Inner\" @0..30\n",
            "      Field c0:field0 \"value\" : i64 @0..30\n",
            "      CopyConstructor\n",
            "        Unavailable\n",
            "      CopyAssignment\n",
            "        Unavailable\n",
            "    Class c1 module m0 \"Outer\" @0..30\n",
            "      Field c1:field0 \"inner\" : class c0 @0..30\n",
            "      Initializer c1:init0(i64) @0..30\n",
            "      CopyConstructor\n",
            "        Unavailable\n",
            "      CopyAssignment\n",
            "        Unavailable\n",
            "      DestructionPlan\n",
            "        Field c1:field0\n",
            "      Method c1:method0 \"get\" readonly () -> i64 @0..30\n",
            "  Declarations\n",
            "    Declaration f0 module m0 \"main\" internal @0..30\n",
            "      Signature () -> i64\n",
            "  Definitions\n",
            "    Definition f0 @0..30\n",
            "      Parameters\n",
            "      Storage\n",
            "        f0:s0 local f0:l0 \"object\" : class c1 @0..30\n",
            "      Values\n",
            "        f0:v0 : i64 @26..27\n",
            "        f0:v1 : i64 @0..30\n",
            "        f0:v2 : i64 @0..30\n",
            "      EntryBlock f0:b0\n",
            "      Blocks\n",
            "        f0:b0 @17..30\n",
            "          storage-live f0:s0 @0..30\n",
            "          f0:v0 = const.i64 7 : i64 @26..27\n",
            "          initialize f0:s0 with c1:init0(value(f0:v0)) @0..30\n",
            "          f0:v1 = load f0:s0.field(c1:field0).field(c0:field0) : i64 @0..30\n",
            "          f0:v2 = call direct c1:method0 on f0:s0 mutable origin exact(f0:s0 : c1)() @0..30\n",
            "          cleanup f0:s0 as c1 @0..30\n",
            "          storage-dead f0:s0 @0..30\n",
            "          return f0:v0 @19..28\n",
            "  MemberDefinitions\n",
            "    MemberDefinition c1:init0 @0..30\n",
            "      Receiver c1:init0:s0\n",
            "      Parameters c1:init0:s1\n",
            "      Storage\n",
            "        c1:init0:s0 receiver c1:init0:self \"self\" : class c1 @0..30\n",
            "        c1:init0:s1 parameter c1:init0:p0 \"parameter0\" : i64 @0..30\n",
            "      Values\n",
            "      EntryBlock c1:init0:b0\n",
            "      Blocks\n",
            "        c1:init0:b0 @0..30\n",
            "          return @0..30\n",
        )
    );
    assert_eq!(dump, dump_mir(&program));
    assert!(!dump.contains("offset"));
}

#[test]
fn rejects_object_rvalues_and_bad_initialization_targets() {
    let (mut object_value, ids) = object_mir();
    let function = object_value
        .definitions
        .get_mut_for_test(object_value.entry_function)
        .unwrap();
    function.values[1].ty = MirType::Class(ids.outer);
    let assignment = function.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Assign(assignment)
                if matches!(assignment.rvalue.kind, MirRvalueKind::Load(_)) =>
            {
                Some(assignment)
            }
            _ => None,
        })
        .expect("expected projected load");
    assignment.rvalue.ty = MirType::Class(ids.outer);
    assignment.rvalue.kind = MirRvalueKind::Load(ids.object_storage.into());
    let errors = messages(&object_value);
    assert!(errors
        .iter()
        .any(|message| message.contains("must have a scalar value type")));
    assert!(errors
        .iter()
        .any(|message| message.contains("load source must have scalar")));

    let (mut bad_target, ids) = object_mir();
    let function = bad_target
        .definitions
        .get_mut_for_test(bad_target.entry_function)
        .unwrap();
    let initialize = function.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Initialize(initialize) => Some(initialize),
            _ => None,
        })
        .expect("expected initializer");
    initialize.destination = MirPlace::base(ids.object_storage).project_field(ids.outer_inner);
    assert!(messages(&bad_target)
        .iter()
        .any(|message| message.contains("wrong class type")));
}

#[test]
fn lowers_typed_source_objects_into_verified_mir() {
    let program = lower_text(concat!(
        "class Box { value: i64; init(value: i64) { self.value = value; } ",
        "mut fn set(value: i64) -> unit { self.value = value; } ",
        "fn get() -> i64 { return self.value; } }\n",
        "fn main() -> i64 { var value: Box = Box(1); value.set(2); return value.get(); }\n",
    ));

    assert!(verify_mir(&program).is_ok());
    let class = program.class(ClassId::new(0)).unwrap();
    assert_eq!(class.fields[0].id, FieldId::new(class.id, 0));
    assert_eq!(class.initializers[0].id, InitializerId::new(class.id, 0));
    assert_eq!(class.methods[0].id, MethodId::new(class.id, 0));
    assert_eq!(class.methods[1].id, MethodId::new(class.id, 1));
    assert!(program
        .member_definition(class.initializers[0].id.into())
        .is_some());
    assert!(program
        .member_definition(class.methods[1].id.into())
        .is_some());

    let main = program.definitions.get(program.entry_function).unwrap();
    assert!(matches!(main.storage[0].ty, MirType::Class(id) if id == class.id));
    assert!(main
        .values
        .iter()
        .all(|value| value.ty != MirType::Class(class.id)));
    assert!(main.body.blocks[0]
        .instructions
        .iter()
        .any(|instruction| matches!(instruction, MirInstruction::Initialize(_))));
}

#[test]
fn source_object_mir_dump_is_exact_and_identity_based() {
    let program = lower_text(concat!(
        "class Box { value: i64; init(value: i64) { self.value = value; } fn get() -> i64 { return self.value; } }\n",
        "fn main() -> i64 { var value: Box = Box(1); return value.get(); }\n",
    ));

    assert_eq!(
        dump_mir(&program),
        concat!(
            "MirProgram @0..172\n",
            "  SelectedModule m0\n",
            "  Modules\n",
            "    Module m0 main source 0 provider provider0 package package0\n",
            "  Entry f0\n",
            "  Classes\n",
            "    Class c0 module m0 \"Box\" @0..105\n",
            "      Field c0:field0 \"value\" : i64 @12..23\n",
            "      Initializer c0:init0(i64) @24..64\n",
            "      CopyConstructor\n",
            "        Synthesized c0\n",
            "          Primitive c0:field0\n",
            "      CopyAssignment\n",
            "        Synthesized c0\n",
            "          Primitive c0:field0\n",
            "      Method c0:method0 \"get\" readonly () -> i64 @65..103\n",
            "  Declarations\n",
            "    Declaration f0 module m0 \"main\" internal @106..171\n",
            "      Signature () -> i64\n",
            "  Definitions\n",
            "    Definition f0 @106..171\n",
            "      Parameters\n",
            "      Storage\n",
            "        f0:s0 local f0:l0 \"value\" : class c0 @125..149\n",
            "      Values\n",
            "        f0:v0 : i64 @146..147\n",
            "        f0:v1 : i64 @157..168\n",
            "      EntryBlock f0:b0\n",
            "      Blocks\n",
            "        f0:b0 @123..171\n",
            "          storage-live f0:s0 @125..149\n",
            "          f0:v0 = const.i64 1 : i64 @146..147\n",
            "          initialize f0:s0 with c0:init0(value(f0:v0)) @142..148\n",
            "          f0:v1 = call direct c0:method0 on f0:s0 mutable origin exact(f0:s0 : c0)() @157..168\n",
            "          cleanup f0:s0 as c0 @150..169\n",
            "          storage-dead f0:s0 @150..169\n",
            "          return f0:v1 @150..169\n",
            "  MemberDefinitions\n",
            "    MemberDefinition c0:init0 @24..64\n",
            "      Receiver c0:init0:s0\n",
            "      Parameters c0:init0:s1\n",
            "      Storage\n",
            "        c0:init0:s0 receiver c0:init0:self \"self\" : class c0 @41..64\n",
            "        c0:init0:s1 parameter c0:init0:p0 \"value\" : i64 @29..39\n",
            "      Values\n",
            "        c0:init0:v0 : i64 @56..61\n",
            "      EntryBlock c0:init0:b0\n",
            "      Blocks\n",
            "        c0:init0:b0 @41..64\n",
            "          c0:init0:v0 = load c0:init0:s1 : i64 @56..61\n",
            "          store c0:init0:s0.field(c0:field0), c0:init0:v0 @43..62\n",
            "          return @41..64\n",
            "    MemberDefinition c0:method0 @65..103\n",
            "      Receiver c0:method0:s0\n",
            "      Parameters\n",
            "      Storage\n",
            "        c0:method0:s0 receiver c0:method0:self \"self\" : class c0 @81..103\n",
            "      Values\n",
            "        c0:method0:v0 : i64 @90..100\n",
            "      EntryBlock c0:method0:b0\n",
            "      Blocks\n",
            "        c0:method0:b0 @81..103\n",
            "          c0:method0:v0 = load c0:method0:s0.field(c0:field0) : i64 @90..100\n",
            "          return c0:method0:v0 @83..101\n",
        )
    );
}

#[test]
fn preserves_object_storage_and_call_order_across_nested_control_flow() {
    let program = lower_text(concat!(
        "fn mark(value: i64) -> i64 { return value; }\n",
        "class Sample { amount: i64; count: u64; small: u8; ratio: f64; enabled: bool; ",
        "init(amount: i64, count: u64, small: u8, ratio: f64, enabled: bool) { ",
        "self.amount = amount; self.count = count; self.small = small; ",
        "self.ratio = ratio; self.enabled = enabled; } ",
        "fn read() -> i64 { return self.amount; } ",
        "mut fn update(first: i64, second: i64) -> unit { self.amount = first + second; } ",
        "mut fn relay(value: i64) -> unit { self.update(value, self.read()); } }\n",
        "fn main() -> i64 { ",
        "var first: Sample = Sample(mark(1), 2u, 3u8, 4.0, true); ",
        "{ var second: Sample = Sample(mark(5), 6u, 7u8, 8.0, false); ",
        "if (true) { first.update(mark(9), mark(10)); } } ",
        "first.relay(mark(11)); return first.read(); }\n",
    ));

    verify_mir(&program).unwrap();
    let class = program.class(ClassId::new(0)).unwrap();
    assert_eq!(
        class
            .fields
            .iter()
            .map(|field| field.ty)
            .collect::<Vec<_>>(),
        vec![
            MirType::I64,
            MirType::U64,
            MirType::U8,
            MirType::F64,
            MirType::Bool,
        ]
    );
    assert_eq!(
        class.methods[0].kind.receiver_access(),
        Some(MirReceiverAccess::ReadOnly)
    );
    assert_eq!(
        class.methods[1].kind.receiver_access(),
        Some(MirReceiverAccess::Mutable)
    );
    assert_eq!(
        class.methods[2].kind.receiver_access(),
        Some(MirReceiverAccess::Mutable)
    );

    let main = program.definitions.get(program.entry_function).unwrap();
    let object_storage = main
        .storage
        .iter()
        .filter(|storage| storage.ty == MirType::Class(class.id))
        .collect::<Vec<_>>();
    assert_eq!(object_storage.len(), 2);
    assert!(main.values.iter().all(|value| value.ty.is_scalar_value()));
    assert_eq!(
        calls_in_source_order(&main.body),
        vec![
            MirCallTarget::Direct(FunctionId::new(0)),
            MirCallTarget::Direct(FunctionId::new(0)),
            MirCallTarget::Direct(FunctionId::new(0)),
            MirCallTarget::Direct(FunctionId::new(0)),
            MirCallTarget::Method(MirMethodCallTarget::Direct(MethodId::new(class.id, 1))),
            MirCallTarget::Direct(FunctionId::new(0)),
            MirCallTarget::Method(MirMethodCallTarget::Direct(MethodId::new(class.id, 2))),
            MirCallTarget::Method(MirMethodCallTarget::Direct(MethodId::new(class.id, 0))),
        ]
    );

    let relay = program
        .member_definition(MethodId::new(class.id, 2).into())
        .unwrap();
    assert_eq!(
        calls_in_source_order(&relay.body),
        vec![
            MirCallTarget::Method(MirMethodCallTarget::Direct(MethodId::new(class.id, 0))),
            MirCallTarget::Method(MirMethodCallTarget::Direct(MethodId::new(class.id, 1))),
        ]
    );
    for call in relay
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match instruction {
            MirInstruction::Call(call) => Some(call),
            _ => None,
        })
    {
        assert_eq!(
            call.receiver
                .as_ref()
                .and_then(MirCallReceiver::as_method)
                .unwrap()
                .place
                .base,
            MirPlaceBase::Storage(relay.receiver.unwrap())
        );
    }
}

#[test]
fn source_lowered_objects_are_accepted_by_the_existing_backend() {
    let program = lower_text(concat!(
        "class Box { value: i64; init(value: i64) { self.value = value; } ",
        "fn get() -> i64 { return self.value; } }\n",
        "fn main() -> i64 { var value: Box = Box(42); return value.get(); }\n",
    ));

    let assembly = crate::test_support::emit_assembly_without_runtime_trace(
        crate::backend::Target::X86_64SysV,
        &program,
    )
    .expect("source-lowered object MIR must be accepted by the object backend");
    assert!(assembly.contains("call .Lska.class.main.Box.c0.init.i0"));
    assert!(assembly.contains("call .Lska.class.main.Box.c0.method.get.m0"));
}

fn calls_in_source_order(body: &MirBody) -> Vec<MirCallTarget> {
    body.blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match instruction {
            MirInstruction::Call(call) => Some(call.target),
            _ => None,
        })
        .collect()
}
