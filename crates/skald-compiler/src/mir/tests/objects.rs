use super::{object_fixtures::*, *};

#[test]
fn verifies_class_metadata_nested_places_initialization_and_receiver_calls() {
    let (program, _) = object_mir();
    assert!(verify_mir(&program).is_ok());
}

#[test]
fn dumps_object_metadata_and_projected_places_deterministically() {
    let (program, _) = object_mir();
    let dump = dump_mir(&program);

    assert_eq!(
        dump,
        concat!(
            "MirProgram @0..30\n",
            "  Entry f0\n",
            "  Classes\n",
            "    Class c0 \"Inner\" @0..30\n",
            "      Field c0:field0 \"value\" : i64 @0..30\n",
            "    Class c1 \"Outer\" @0..30\n",
            "      Field c1:field0 \"inner\" : class c0 @0..30\n",
            "      Initializer c1:init0(i64) @0..30\n",
            "      Method c1:method0 \"get\" readonly () -> i64 @0..30\n",
            "  Declarations\n",
            "    Declaration f0 \"main\" internal @0..30\n",
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
            "          f0:v0 = const.i64 7 : i64 @26..27\n",
            "          initialize f0:s0 with c1:init0(f0:v0) @0..30\n",
            "          f0:v1 = load f0:s0.field(c1:field0).field(c0:field0) : i64 @0..30\n",
            "          f0:v2 = call c1:method0 on f0:s0() @0..30\n",
            "          return f0:v0 @19..28\n",
        )
    );
    assert_eq!(dump, dump_mir(&program));
    assert!(!dump.contains("offset"));
}

#[test]
fn rejects_foreign_and_non_class_field_projections() {
    let (mut foreign, ids) = object_mir();
    let function = foreign
        .definitions
        .get_mut_for_test(foreign.entry_function)
        .unwrap();
    let MirInstruction::Assign(assignment) = &mut function.body.blocks[0].instructions[2] else {
        panic!("expected projected load");
    };
    assignment.rvalue.kind =
        MirRvalueKind::Load(MirPlace::base(ids.object_storage).project_field(ids.inner_value));
    assert!(messages(&foreign)
        .iter()
        .any(|message| message.contains("belongs to the wrong class")));

    let (mut scalar, ids) = object_mir();
    let function = scalar
        .definitions
        .get_mut_for_test(scalar.entry_function)
        .unwrap();
    let MirInstruction::Assign(assignment) = &mut function.body.blocks[0].instructions[2] else {
        panic!("expected projected load");
    };
    assignment.rvalue.kind = MirRvalueKind::Load(
        MirPlace::base(ids.object_storage)
            .project_field(ids.outer_inner)
            .project_field(ids.inner_value)
            .project_field(ids.inner_value),
    );
    assert!(messages(&scalar)
        .iter()
        .any(|message| message.contains("has a non-class base")));
}

#[test]
fn rejects_object_rvalues_and_bad_initialization_targets() {
    let (mut object_value, ids) = object_mir();
    let function = object_value
        .definitions
        .get_mut_for_test(object_value.entry_function)
        .unwrap();
    function.values[1].ty = MirType::Class(ids.outer);
    let MirInstruction::Assign(assignment) = &mut function.body.blocks[0].instructions[2] else {
        panic!("expected projected load");
    };
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
    let MirInstruction::Initialize(initialize) = &mut function.body.blocks[0].instructions[1]
    else {
        panic!("expected initializer");
    };
    initialize.destination = MirPlace::base(ids.object_storage).project_field(ids.outer_inner);
    assert!(messages(&bad_target)
        .iter()
        .any(|message| message.contains("wrong class type")));
}

#[test]
fn rejects_missing_wrong_receiver_and_mismatched_arguments() {
    let (mut missing, _) = object_mir();
    let function = missing
        .definitions
        .get_mut_for_test(missing.entry_function)
        .unwrap();
    let MirInstruction::Call(call) = &mut function.body.blocks[0].instructions[3] else {
        panic!("expected method call");
    };
    call.receiver = None;
    assert!(messages(&missing)
        .iter()
        .any(|message| message.contains("requires a receiver")));

    let (mut wrong, ids) = object_mir();
    let function = wrong
        .definitions
        .get_mut_for_test(wrong.entry_function)
        .unwrap();
    let MirInstruction::Call(call) = &mut function.body.blocks[0].instructions[3] else {
        panic!("expected method call");
    };
    call.receiver = Some(MirPlace::base(ids.object_storage).project_field(ids.outer_inner));
    assert!(messages(&wrong)
        .iter()
        .any(|message| message.contains("receiver has the wrong class type")));

    let (mut arguments, _) = object_mir();
    let function = arguments
        .definitions
        .get_mut_for_test(arguments.entry_function)
        .unwrap();
    let MirInstruction::Initialize(initialize) = &mut function.body.blocks[0].instructions[1]
    else {
        panic!("expected initializer");
    };
    initialize.arguments.clear();
    assert!(messages(&arguments)
        .iter()
        .any(|message| message.contains("initializer has 0 arguments but requires 1")));
}

#[test]
fn rejects_member_metadata_with_the_wrong_owner() {
    let (mut program, ids) = object_mir();
    program.classes.entries_mut_for_test()[1].fields[0].id = FieldId::new(ids.inner, 0);
    assert!(messages(&program)
        .iter()
        .any(|message| message.contains("field table index 0")));
}
