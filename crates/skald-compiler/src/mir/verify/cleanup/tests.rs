use crate::{
    identity::{ClassId, FunctionId, MethodId},
    mir::{verify_mir, MirCleanup, MirInstruction, MirPlace, MirPlaceBase, MirProgram, StorageId},
    test_support::lower_source_to_mir,
};

fn cleanup_program() -> MirProgram {
    lower_source_to_mir(concat!(
        "class Resource { init() {} }\n",
        "fn main() -> i64 { var resource: Resource = Resource(); return 0; }\n",
    ))
}

fn cleanup_mut(program: &mut MirProgram) -> &mut MirCleanup {
    let function = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    function.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Cleanup(cleanup) => Some(cleanup),
            _ => None,
        })
        .unwrap()
}

fn messages(program: &MirProgram) -> Vec<String> {
    verify_mir(program)
        .unwrap_err()
        .iter()
        .map(|error| error.message.clone())
        .collect()
}

#[test]
fn structural_cleanup_validation_retains_exact_target_diagnostics() {
    let mut program = cleanup_program();
    cleanup_mut(&mut program).target = ClassId::new(1);

    let messages = messages(&program);
    assert!(messages
        .iter()
        .any(|message| message == "cleanup target c1 is not declared"));
    assert!(messages
        .iter()
        .any(|message| message == "cleanup destination has the wrong class type"));
}

#[test]
fn structural_cleanup_validation_rejects_non_owning_foreign_and_scalar_places() {
    let mut non_owning = cleanup_program();
    let cleanup = cleanup_mut(&mut non_owning);
    cleanup.destination.base = MirPlaceBase::AliasParameter(cleanup.destination.base.storage());
    assert!(messages(&non_owning)
        .iter()
        .any(|message| message == "cleanup destination must be owning storage"));

    let mut foreign = cleanup_program();
    cleanup_mut(&mut foreign).destination = MirPlace::base(StorageId::new(FunctionId::new(99), 0));
    assert!(messages(&foreign)
        .iter()
        .any(|message| message == "place base f99:s0 is not declared in this function"));

    let mut scalar = lower_source_to_mir(concat!(
        "class Empty { init() {} }\n",
        "fn main() -> i64 { var scalar: i64 = 0; var empty: Empty = Empty(); return 0; }\n",
    ));
    let function = scalar
        .definitions
        .get_mut_for_test(scalar.entry_function)
        .unwrap();
    function.body.blocks[0]
        .instructions
        .push(MirInstruction::Cleanup(MirCleanup {
            destination: function.storage[0].id.into(),
            target: ClassId::new(0),
            span: function.span,
        }));
    assert!(messages(&scalar)
        .iter()
        .any(|message| message == "cleanup destination must have class type"));
}

#[test]
fn structural_cleanup_validation_rejects_read_only_receiver_access() {
    let mut program = lower_source_to_mir(concat!(
        "class Resource { init() {} fn inspect() -> unit {} }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    let method = MethodId::new(ClassId::new(0), 0);
    let definition = program
        .member_definitions
        .get_mut_for_test(method.into())
        .unwrap();
    definition.body.blocks[0]
        .instructions
        .push(MirInstruction::Cleanup(MirCleanup {
            destination: definition.receiver.unwrap().into(),
            target: ClassId::new(0),
            span: definition.span,
        }));

    assert!(messages(&program)
        .iter()
        .any(|message| message == "cleanup destination requires mutable access"));
}

#[test]
fn cleanup_liveness_retains_the_exact_duplicate_destruction_diagnostic() {
    let mut program = cleanup_program();
    let cleanup = cleanup_mut(&mut program).clone();
    let function = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    function.body.blocks[0]
        .instructions
        .push(MirInstruction::Cleanup(cleanup));

    assert!(messages(&program)
        .iter()
        .any(|message| message == "cleanup destination is destroyed more than once"));
}

#[test]
fn cleanup_liveness_rejects_dead_destinations_and_live_normal_exit_roots() {
    let mut dead = cleanup_program();
    let function = dead
        .definitions
        .get_mut_for_test(dead.entry_function)
        .unwrap();
    let cleanup_index = function.body.blocks[0]
        .instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::Cleanup(_)))
        .unwrap();
    let cleanup = function.body.blocks[0].instructions.remove(cleanup_index);
    function.body.blocks[0].instructions.insert(0, cleanup);
    assert!(messages(&dead)
        .iter()
        .any(|message| message == "cleanup destination is not live"));

    let mut live_on_exit = cleanup_program();
    let function = live_on_exit
        .definitions
        .get_mut_for_test(live_on_exit.entry_function)
        .unwrap();
    function.body.blocks[0]
        .instructions
        .retain(|instruction| !matches!(instruction, MirInstruction::Cleanup(_)));
    assert!(messages(&live_on_exit)
        .iter()
        .any(|message| message == "owning local remains live on normal return"));
}

#[test]
fn cleanup_liveness_checks_each_control_flow_path() {
    let mut program = lower_source_to_mir(concat!(
        "class Resource { init() {} }\n",
        "fn choose(flag: bool) -> i64 {\n",
        "  if (flag) { var left: Resource = Resource(); }\n",
        "  else { var right: Resource = Resource(); }\n",
        "  return 0;\n",
        "}\n",
        "fn main() -> i64 { return choose(false); }\n",
    ));
    let choose = program
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    choose.body.blocks[2]
        .instructions
        .retain(|instruction| !matches!(instruction, MirInstruction::Cleanup(_)));

    assert!(messages(&program)
        .iter()
        .any(|message| message == "owning local remains live on normal return"));
}
