use super::*;

fn function_id(program: &MirProgram, name: &str) -> FunctionId {
    program
        .declarations
        .iter()
        .find(|declaration| declaration.name == name)
        .unwrap_or_else(|| panic!("fixture function `{name}` must be declared"))
        .id
}

fn verifier_errors(program: &MirProgram) -> String {
    verify_mir(program)
        .expect_err("mutated produced-field MIR must fail verification")
        .to_string()
}

fn field_fixture() -> MirProgram {
    lower_text(concat!(
        "class Leaf {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  fn read() -> i64 { return self.value; }\n",
        "  destroy {}\n",
        "}\n",
        "class Holder {\n",
        "  first: Leaf; second: Leaf;\n",
        "  init(value: i64) { self.first = Leaf(value); self.second = Leaf(value + 1); }\n",
        "  destroy {}\n",
        "}\n",
        "fn inspect(ref leaf: Leaf) -> i64 { return leaf.read(); }\n",
        "fn main() -> i64 { return inspect(Holder(7).first); }\n",
    ))
}

fn entry_temporary(program: &MirProgram) -> StorageId {
    program
        .definitions
        .get(program.entry_function)
        .unwrap()
        .storage
        .iter()
        .find(|storage| storage.kind == MirStorageKind::Temporary)
        .expect("fixture must materialize one produced root")
        .id
}

fn entry_view_mut(program: &mut MirProgram) -> &mut MirObjectView {
    let entry = program.entry_function;
    program
        .definitions
        .get_mut_for_test(entry)
        .unwrap()
        .body
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call) => {
                call.arguments
                    .iter_mut()
                    .find_map(|argument| match argument {
                        MirArgument::View(view)
                            if view.provenance == MirViewProvenance::Produced =>
                        {
                            Some(view)
                        }
                        _ => None,
                    })
            }
            _ => None,
        })
        .expect("fixture must pass one produced field view")
}

fn produced_cleanup_mut(function: &mut MirFunctionDefinition) -> &mut MirEndFullExpression {
    function
        .body
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::EndFullExpression(end) if !end.temporaries.is_empty() => Some(end),
            _ => None,
        })
        .expect("fixture must clean its produced root")
}

#[test]
fn produced_field_view_uses_one_complete_live_root_and_cleans_after_consumption() {
    let program = field_fixture();
    verify_mir(&program).expect("produced-field fixture must verify");
    let main = program.definitions.get(program.entry_function).unwrap();
    let temporary = entry_temporary(&program);
    let instructions = &main.body.blocks[0].instructions;

    let live = instructions
        .iter()
        .position(|instruction| {
            matches!(instruction, MirInstruction::StorageLive(operation)
            if operation.storage == temporary)
        })
        .unwrap();
    let initialize = instructions
        .iter()
        .position(|instruction| {
            matches!(instruction, MirInstruction::Initialize(operation)
            if operation.destination == MirPlace::base(temporary))
        })
        .unwrap();
    let call = instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::Call(_)))
        .unwrap();
    let cleanup = instructions
        .iter()
        .position(|instruction| {
            matches!(instruction, MirInstruction::EndFullExpression(end)
            if end.temporaries.iter().any(|cleanup|
                cleanup.destination == MirPlace::base(temporary)))
        })
        .unwrap();
    let dead = instructions
        .iter()
        .position(|instruction| {
            matches!(instruction, MirInstruction::StorageDead(operation)
            if operation.storage == temporary)
        })
        .unwrap();
    assert!(live < initialize && initialize < call && call < cleanup && cleanup < dead);

    let MirInstruction::Call(call) = &instructions[call] else {
        unreachable!()
    };
    let MirArgument::View(view) = &call.arguments[0] else {
        panic!("produced class field must remain a view argument")
    };
    assert_eq!(view.access, MirAliasAccess::ReadOnly);
    assert_eq!(view.provenance, MirViewProvenance::Produced);
    assert_eq!(view.source.base.local_storage(), Some(temporary));
    assert!(matches!(
        view.source.projections.as_slice(),
        [MirPlaceProjection::Field(_)]
    ));
    assert!(matches!(
        view.origin.as_ref(),
        MirObjectOrigin::Exact { complete, dynamic_class }
            if complete == &view.source && *dynamic_class == ClassId::new(0)
    ));
    assert!(!instructions.iter().any(|instruction| {
        matches!(instruction, MirInstruction::CopyConstruct(copy)
            if copy.source == view.source)
    }));
}

#[test]
fn verifier_rejects_produced_field_lifetime_and_cleanup_corruption() {
    let valid = field_fixture();
    let temporary = entry_temporary(&valid);

    let mut missing = valid.clone();
    let entry = missing.entry_function;
    produced_cleanup_mut(missing.definitions.get_mut_for_test(entry).unwrap())
        .temporaries
        .clear();
    assert!(verifier_errors(&missing).contains("full-expression temporaries must be cleaned"));

    let mut duplicate = valid.clone();
    let entry = duplicate.entry_function;
    let end = produced_cleanup_mut(duplicate.definitions.get_mut_for_test(entry).unwrap());
    end.temporaries.push(end.temporaries[0].clone());
    assert!(verifier_errors(&duplicate).contains("cleanup destination is not live"));

    let mut premature = valid.clone();
    let entry = premature.entry_function;
    let function = premature.definitions.get_mut_for_test(entry).unwrap();
    let instructions = &mut function.body.blocks[0].instructions;
    let call = instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::Call(_)))
        .unwrap();
    let cleanup = instructions
        .iter()
        .position(|instruction| {
            matches!(instruction, MirInstruction::EndFullExpression(end)
            if !end.temporaries.is_empty())
        })
        .unwrap();
    let boundary = instructions.remove(cleanup);
    instructions.insert(call, boundary);
    assert!(verifier_errors(&premature).contains("object view source is not live"));

    let mut before_initialization = valid.clone();
    let entry = before_initialization.entry_function;
    let function = before_initialization
        .definitions
        .get_mut_for_test(entry)
        .unwrap();
    let instructions = &mut function.body.blocks[0].instructions;
    let initialize = instructions
        .iter()
        .position(|instruction| {
            matches!(instruction, MirInstruction::Initialize(operation)
            if operation.destination == MirPlace::base(temporary))
        })
        .unwrap();
    let call = instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::Call(_)))
        .unwrap();
    instructions.swap(initialize, call);
    assert!(verifier_errors(&before_initialization).contains("object view source is not live"));

    let mut post_cleanup = valid;
    let entry = post_cleanup.entry_function;
    let function = post_cleanup.definitions.get_mut_for_test(entry).unwrap();
    let instructions = &mut function.body.blocks[0].instructions;
    let call = instructions
        .iter()
        .find(|instruction| matches!(instruction, MirInstruction::Call(_)))
        .unwrap()
        .clone();
    let cleanup = instructions
        .iter()
        .position(|instruction| {
            matches!(instruction, MirInstruction::EndFullExpression(end)
            if !end.temporaries.is_empty())
        })
        .unwrap();
    instructions.insert(cleanup + 1, call);
    assert!(verifier_errors(&post_cleanup).contains("object view source is not live"));
}

#[test]
fn verifier_rejects_produced_field_origin_projection_and_access_corruption() {
    let valid = field_fixture();

    let mut root_origin = valid.clone();
    let temporary = entry_temporary(&root_origin);
    let view = entry_view_mut(&mut root_origin);
    let MirObjectOrigin::Exact { complete, .. } = view.origin.as_mut() else {
        unreachable!()
    };
    *complete = MirPlace::base(temporary);
    assert!(verifier_errors(&root_origin).contains("exact origin has the wrong dynamic class"));

    let mut wrong_path = valid.clone();
    let view = entry_view_mut(&mut wrong_path);
    let MirObjectOrigin::Exact { complete, .. } = view.origin.as_mut() else {
        unreachable!()
    };
    *complete = MirPlace::base(temporary).project_field(FieldId::new(ClassId::new(1), 1));
    assert!(verifier_errors(&wrong_path).contains("not an ancestor"));

    let mut invalid_projection = valid.clone();
    entry_view_mut(&mut invalid_projection).source.projections[0] =
        MirPlaceProjection::Field(FieldId::new(ClassId::new(1), 99));
    assert!(verifier_errors(&invalid_projection).contains("field projection"));

    let mut mutable = valid;
    entry_view_mut(&mut mutable).access = MirAliasAccess::Mutable;
    assert!(verifier_errors(&mutable).contains("produced object view must be read-only"));
}

#[test]
fn nested_produced_field_roots_clean_in_reverse_completion_order() {
    let mut program = lower_text(concat!(
        "class Leaf { value: i64; init(value: i64) { self.value = value; } destroy {} }\n",
        "class Holder { leaf: Leaf; init(value: i64) { self.leaf = Leaf(value); } destroy {} }\n",
        "fn inspect(ref first: Leaf, ref second: Leaf) -> i64 { return first.value + second.value; }\n",
        "fn main() -> i64 { return inspect(Holder(1).leaf, Holder(2).leaf); }\n",
    ));
    verify_mir(&program).expect("multiple produced fields must verify");
    let main = program.definitions.get(program.entry_function).unwrap();
    let temporaries = main
        .storage
        .iter()
        .filter(|storage| storage.kind == MirStorageKind::Temporary)
        .map(|storage| storage.id)
        .collect::<Vec<_>>();
    assert_eq!(temporaries.len(), 2);
    let cleanup = main
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::EndFullExpression(end) if end.temporaries.len() == 2 => Some(end),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        cleanup
            .temporaries
            .iter()
            .map(|cleanup| cleanup.destination.base.expect_local_storage())
            .collect::<Vec<_>>(),
        [temporaries[1], temporaries[0]]
    );

    let entry = program.entry_function;
    produced_cleanup_mut(program.definitions.get_mut_for_test(entry).unwrap())
        .temporaries
        .swap(0, 1);
    assert!(verifier_errors(&program).contains("reverse completion order"));
}

#[test]
fn produced_field_control_paths_are_epoch_local_and_terminating_paths_do_not_unwind() {
    let program = lower_text(concat!(
        "class State {\n",
        "  truth: bool; values: i64[];\n",
        "  init(truth: bool) { self.truth = truth; self.values = i64[]{1}; }\n",
        "  destroy {}\n",
        "}\n",
        "fn choose(flag: bool) -> bool {\n",
        "  if (flag && State(true).truth) { return true; }\n",
        "  elif (State(false).truth) { return true; }\n",
        "  return false || State(true).truth;\n",
        "}\n",
        "fn repeat(limit: i64) -> i64 {\n",
        "  var count: i64 = 0;\n",
        "  while (count < limit && State(true).truth) { count = count + 1; }\n",
        "  return count;\n",
        "}\n",
        "fn indexed(index: i64) -> i64 { return State(true).values[index]; }\n",
        "fn produce(divisor: i64) -> i64 { return State(40 / divisor == 40).values[0]; }\n",
        "fn main() -> i64 { if (choose(false)) { return repeat(2); } return indexed(0); }\n",
    ));
    verify_mir(&program).expect("produced field control paths must verify");

    let choose = program
        .definitions
        .get(function_id(&program, "choose"))
        .unwrap();
    let repeat = program
        .definitions
        .get(function_id(&program, "repeat"))
        .unwrap();
    assert!(!choose.body.path_conditions.is_empty());
    assert!(!repeat.body.path_conditions.is_empty());
    assert!(
        choose
            .storage
            .iter()
            .filter(|storage| storage.kind == MirStorageKind::Temporary)
            .count()
            >= 3
    );
    assert_eq!(
        repeat
            .storage
            .iter()
            .filter(|storage| storage.kind == MirStorageKind::Temporary)
            .count(),
        1
    );

    let indexed = program
        .definitions
        .get(function_id(&program, "indexed"))
        .unwrap();
    let termination = indexed
        .body
        .blocks
        .iter()
        .find(|block| {
            matches!(
                block.terminator,
                Some(MirTerminator::Terminate {
                    reason: MirTerminationReason::ArrayIndexOutOfBounds,
                    ..
                })
            )
        })
        .expect("checked field indexing must retain its abrupt failure block");
    assert!(!termination
        .instructions
        .iter()
        .any(|instruction| matches!(instruction, MirInstruction::EndFullExpression(_))));

    let produce = program
        .definitions
        .get(function_id(&program, "produce"))
        .unwrap();
    let production_failure = produce
        .body
        .blocks
        .iter()
        .find(|block| {
            matches!(
                block.terminator,
                Some(MirTerminator::Terminate {
                    reason: MirTerminationReason::IntegerDivisionByZero,
                    ..
                })
            )
        })
        .expect("failing producer input must retain its abrupt failure block");
    assert!(!production_failure
        .instructions
        .iter()
        .any(|instruction| matches!(instruction, MirInstruction::EndFullExpression(_))));
}

#[test]
fn verifier_rejects_produced_field_cleanup_on_a_skipped_logical_path() {
    let mut program = lower_text(concat!(
        "class Holder { value: bool; init(value: bool) { self.value = value; } destroy {} }\n",
        "fn choose(flag: bool) -> bool { return flag && Holder(true).value; }\n",
        "fn main() -> i64 { if (choose(false)) { return 1; } return 0; }\n",
    ));
    verify_mir(&program).expect("conditional produced-field seed must verify");
    let choose_id = function_id(&program, "choose");
    let choose = program.definitions.get(choose_id).unwrap();
    let temporary = choose
        .storage
        .iter()
        .find(|storage| storage.kind == MirStorageKind::Temporary)
        .unwrap()
        .id;
    let cleanup = choose
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::EndFullExpression(end) => end
                .temporaries
                .iter()
                .find(|cleanup| cleanup.destination == MirPlace::base(temporary))
                .cloned(),
            _ => None,
        })
        .unwrap();
    let skipped = choose.body.path_conditions[0].inactive_predecessor;

    let choose = program.definitions.get_mut_for_test(choose_id).unwrap();
    let block = choose
        .body
        .blocks
        .iter_mut()
        .find(|block| block.id == skipped)
        .unwrap();
    block
        .instructions
        .push(MirInstruction::EndFullExpression(MirEndFullExpression {
            temporaries: vec![cleanup],
            span: block.span,
        }));

    let errors = verifier_errors(&program);
    assert!(
        errors.contains("outside a live lifetime epoch")
            || errors.contains("cleanup destination is not live")
            || errors.contains("conditional owner state"),
        "skipped-path cleanup leakage must fail: {errors}"
    );
}
