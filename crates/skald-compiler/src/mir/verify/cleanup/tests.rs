use crate::{
    identity::{ClassId, FunctionId, InitializerId, MethodId},
    mir::{
        verify_mir, MirArrayInstruction, MirCleanup, MirInstruction, MirPlace, MirPlaceBase,
        MirProgram, MirStorageKind, MirType, StorageId,
    },
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
    cleanup.destination.base =
        MirPlaceBase::AliasParameter(cleanup.destination.base.expect_local_storage());
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

fn array_local_program() -> MirProgram {
    lower_source_to_mir(concat!(
        "fn main() -> i64 {\n",
        "  var values: i64[] = i64[]{1, 2};\n",
        "  var flags: bool[] = bool[]{true};\n",
        "  return values[0];\n",
        "}\n",
    ))
}

#[test]
fn array_local_release_is_required_exactly_once_with_the_exact_type() {
    let program = array_local_program();
    verify_mir(&program).expect("array local mutation seed must verify");
    let function = program.definitions.get(program.entry_function).unwrap();
    let values = function
        .storage
        .iter()
        .find(|storage| storage.name == "values")
        .unwrap();
    let MirType::Array(values_array) = values.ty else {
        unreachable!();
    };
    let flags_array = function
        .storage
        .iter()
        .find_map(|storage| match (storage.name.as_str(), storage.ty) {
            ("flags", MirType::Array(array)) => Some(array),
            _ => None,
        })
        .unwrap();
    let values = values.id;

    let mut missing = program.clone();
    let function = missing
        .definitions
        .get_mut_for_test(missing.entry_function)
        .unwrap();
    for block in &mut function.body.blocks {
        block.instructions.retain(|instruction| {
            !matches!(
                instruction,
                MirInstruction::Array(MirArrayInstruction::Release { owner, array, .. })
                    if *array == values_array
                        && matches!(owner.base, MirPlaceBase::Storage(storage) if storage == values)
            )
        });
    }
    assert!(messages(&missing)
        .iter()
        .any(|message| message == "owning local remains live on normal return"));

    let mut duplicate = program.clone();
    let function = duplicate
        .definitions
        .get_mut_for_test(duplicate.entry_function)
        .unwrap();
    let (block, index, release) = function
        .body
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, block)| {
            block
                .instructions
                .iter()
                .enumerate()
                .find_map(|(index, instruction)| match instruction {
                    MirInstruction::Array(MirArrayInstruction::Release { owner, .. })
                        if matches!(owner.base, MirPlaceBase::Storage(storage) if storage == values) =>
                    {
                        Some((block_index, index, instruction.clone()))
                    }
                    _ => None,
                })
        })
        .unwrap();
    function.body.blocks[block]
        .instructions
        .insert(index + 1, release);
    assert!(messages(&duplicate)
        .iter()
        .any(|message| message == "array owner is released more than once"));

    let mut wrong_type = program;
    let function = wrong_type
        .definitions
        .get_mut_for_test(wrong_type.entry_function)
        .unwrap();
    let release = function
        .body
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Array(MirArrayInstruction::Release { owner, array, .. })
                if matches!(owner.base, MirPlaceBase::Storage(storage) if storage == values) =>
            {
                Some(array)
            }
            _ => None,
        })
        .unwrap();
    *release = flags_array;
    assert!(messages(&wrong_type)
        .iter()
        .any(|message| message == "array release requires an exact matching owner place"));
}

#[test]
fn array_parameter_result_argument_and_conditional_owners_require_transfer_or_cleanup() {
    let source = concat!(
        "fn take(values: i64[]) -> unit {}\n",
        "fn make() -> i64[] { return i64[]{42}; }\n",
        "fn conditional(flag: bool) -> i64 {\n",
        "  if (flag) { var branch: i64[] = i64[]{1}; }\n",
        "  return 0;\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  take(i64[]{1});\n",
        "  var values: i64[] = make();\n",
        "  return values[0] + conditional(false);\n",
        "}\n",
    );
    let program = lower_source_to_mir(source);
    verify_mir(&program).expect("array storage-role mutation seed must verify");

    let mut parameter = program.clone();
    let take = parameter
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let owner = take.parameters[0];
    for block in &mut take.body.blocks {
        block.instructions.retain(|instruction| {
            !matches!(instruction,
                MirInstruction::Array(MirArrayInstruction::Release { owner: place, .. })
                    if matches!(place.base, MirPlaceBase::Storage(storage) if storage == owner))
        });
    }
    assert!(messages(&parameter)
        .iter()
        .any(|message| message == "owning value parameter remains live on normal return"));

    let mut result = program.clone();
    let make = result
        .definitions
        .get_mut_for_test(FunctionId::new(1))
        .unwrap();
    let result_storage = make.return_storage.unwrap();
    for block in &mut make.body.blocks {
        block.instructions.retain(|instruction| {
            !matches!(instruction,
                MirInstruction::Array(MirArrayInstruction::Adopt { destination, .. })
                    if matches!(destination.base, MirPlaceBase::Storage(storage) if storage == result_storage))
        });
    }
    assert!(messages(&result)
        .iter()
        .any(|message| message == "array return storage is not initialized on normal return"));

    let mut argument = program.clone();
    let main = argument
        .definitions
        .get_mut_for_test(argument.entry_function)
        .unwrap();
    let argument_storage = main
        .storage
        .iter()
        .find(|storage| {
            storage.kind == MirStorageKind::Argument && matches!(storage.ty, MirType::Array(_))
        })
        .unwrap()
        .id;
    for block in &mut main.body.blocks {
        block.instructions.retain(|instruction| {
            !matches!(instruction, MirInstruction::Call(call)
                if call.arguments.iter().any(|argument| matches!(argument,
                    crate::mir::MirArgument::OwnedPlace(place)
                        if matches!(place.base, MirPlaceBase::Storage(storage) if storage == argument_storage))))
        });
    }
    assert!(messages(&argument).iter().any(
        |message| message == "caller argument storage remains live without ownership transfer"
    ));

    let mut conditional = program;
    let function = conditional
        .definitions
        .get_mut_for_test(FunctionId::new(2))
        .unwrap();
    let branch = function
        .storage
        .iter()
        .find(|storage| storage.name == "branch")
        .unwrap()
        .id;
    for block in &mut function.body.blocks {
        block.instructions.retain(|instruction| {
            !matches!(instruction,
                MirInstruction::Array(MirArrayInstruction::Release { owner, .. })
                    if matches!(owner.base, MirPlaceBase::Storage(storage) if storage == branch))
        });
    }
    assert!(messages(&conditional)
        .iter()
        .any(|message| message == "owning local remains live on normal return"));
}

#[test]
fn array_fields_distinguish_initialization_from_replacement() {
    let source = concat!(
        "class Holder {\n",
        "  values: i64[];\n",
        "  init() { self.values = i64[]{1}; }\n",
        "  mut fn replace() -> unit { self.values = i64[]{2}; }\n",
        "}\n",
        "fn main() -> i64 { var holder: Holder = Holder(); holder.replace(); return 0; }\n",
    );
    let program = lower_source_to_mir(source);
    verify_mir(&program).expect("array field mutation seed must verify");

    let mut replacement_before_initialization = program.clone();
    let initializer = replacement_before_initialization
        .member_definitions
        .get_mut_for_test(InitializerId::new(ClassId::new(0), 0).into())
        .unwrap();
    let operation = initializer
        .body
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Array(operation @ MirArrayInstruction::Adopt { .. }) => Some(operation),
            _ => None,
        })
        .unwrap();
    let MirArrayInstruction::Adopt {
        destination,
        source,
        array,
        span,
    } = operation.clone()
    else {
        unreachable!();
    };
    *operation = MirArrayInstruction::Replace {
        destination,
        source,
        array,
        authorization: None,
        span,
    };
    assert!(messages(&replacement_before_initialization)
        .iter()
        .any(|message| message == "array replacement destination is not live"));

    let mut duplicate_initialization = program;
    let method = duplicate_initialization
        .member_definitions
        .get_mut_for_test(MethodId::new(ClassId::new(0), 0).into())
        .unwrap();
    let operation = method
        .body
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Array(operation @ MirArrayInstruction::Replace { .. }) => {
                Some(operation)
            }
            _ => None,
        })
        .unwrap();
    let MirArrayInstruction::Replace {
        destination,
        source,
        array,
        span,
        ..
    } = operation.clone()
    else {
        unreachable!();
    };
    *operation = MirArrayInstruction::Adopt {
        destination,
        source,
        array,
        span,
    };
    assert!(messages(&duplicate_initialization)
        .iter()
        .any(|message| message == "initialization destination is already live"));
}

#[test]
fn produced_array_temporaries_are_released_exactly_once() {
    let mut program = lower_source_to_mir(concat!(
        "fn main() -> i64 {\n",
        "  var values: i64[] = i64[]{1, 2, 3};\n",
        "  values[1:3] = values[0:2];\n",
        "  return values[2];\n",
        "}\n",
    ));
    verify_mir(&program).expect("array slice temporary mutation seed must verify");
    let function = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    let (block, index, release) = function
        .body
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, block)| {
            block
                .instructions
                .iter()
                .enumerate()
                .find_map(|(index, instruction)| match instruction {
                    MirInstruction::Array(MirArrayInstruction::Release { owner, .. })
                        if function
                            .storage(owner.base.expect_local_storage())
                            .is_some_and(|storage| storage.kind == MirStorageKind::ArraySlice) =>
                    {
                        Some((block_index, index, instruction.clone()))
                    }
                    _ => None,
                })
        })
        .unwrap();
    function.body.blocks[block]
        .instructions
        .insert(index + 1, release);

    assert!(messages(&program)
        .iter()
        .any(|message| message == "produced array storage must be released exactly once"));
}
