use super::*;
use crate::{
    backend::{emit_assembly, Target},
    passes::run_mir_pipeline,
};

fn exact_owner_program() -> MirProgram {
    lower_text(concat!(
        "class Widget { value: i64; init(value: i64) { self.value = value; } }\n",
        "fn main() -> i64 {\n",
        "  var value: shared Widget = new Widget(7);\n",
        "  return 0;\n",
        "}\n",
    ))
}

fn main_instructions(program: &MirProgram) -> &[MirInstruction] {
    &program
        .definitions
        .get(program.entry_function)
        .unwrap()
        .body
        .blocks[0]
        .instructions
}

fn main_instructions_mut(program: &mut MirProgram) -> &mut Vec<MirInstruction> {
    &mut program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap()
        .body
        .blocks[0]
        .instructions
}

fn has_error(program: &MirProgram, needle: &str) -> bool {
    verify_mir(program)
        .unwrap_err()
        .iter()
        .any(|error| error.message.contains(needle))
}

#[test]
fn lowers_and_verifies_the_first_exact_shared_owner_lifetime() {
    let program = exact_owner_program();
    verify_mir(&program).expect("lowered shared ownership MIR must verify");
    let instructions = main_instructions(&program);
    let allocation = instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::SharedAllocate(_)))
        .unwrap();
    assert!(allocation > 0);
    assert!(instructions[..allocation]
        .iter()
        .any(|instruction| matches!(instruction, MirInstruction::Assign(_))));
    assert!(matches!(
        &instructions[allocation..allocation + 5],
        [
            MirInstruction::SharedAllocate(_),
            MirInstruction::SharedInitialize(_),
            MirInstruction::SharedPublish(_),
            MirInstruction::SharedAdopt(_),
            MirInstruction::EndFullExpression(_),
        ]
    ));
    assert!(instructions
        .iter()
        .skip(allocation + 5)
        .any(|instruction| matches!(instruction, MirInstruction::SharedRelease(_))));
    run_mir_pipeline(program.clone()).expect("shared MIR must survive target-independent passes");
    let assembly = emit_assembly(Target::X86_64SysV, &program)
        .expect("the exact shared lifetime must reach the native backend");
    assert!(assembly.contains("call ska_rt_alloc"));
    assert!(assembly.contains("call ska_rt_free"));
}

#[test]
fn shared_lifetime_dump_is_exact_and_deterministic() {
    let dump = dump_mir(&exact_owner_program());
    assert_eq!(dump, dump_mir(&exact_owner_program()));
    assert!(dump.contains("shared class c0"));
    assert!(dump.contains("shared-allocation"));
    assert!(dump.contains("shared-allocate"));
    assert!(dump.contains("shared-initialize"));
    assert!(dump.contains("shared-publish"));
    assert!(dump.contains("shared-adopt"));
    assert!(dump.contains("end-full-expression"));
    assert!(dump.contains("shared-release"));
}

#[test]
fn lowers_local_copy_and_secure_release_move_assignment_explicitly() {
    let program = lower_text(concat!(
        "class Item { init() {} }\n",
        "fn main() -> i64 {\n",
        "  var source: shared Item = new Item();\n",
        "  var destination: shared Item = source;\n",
        "  destination = destination;\n",
        "  destination = new Item();\n",
        "  return 0;\n",
        "}\n",
    ));
    verify_mir(&program).expect("local owner operations must verify");
    let instructions = main_instructions(&program);
    assert!(instructions.windows(2).any(|window| matches!(
        window,
        [
            MirInstruction::SharedCopy(_),
            MirInstruction::EndFullExpression(_)
        ]
    )));
    assert!(instructions.windows(4).any(|window| matches!(
        window,
        [
            MirInstruction::SharedCopy(_),
            MirInstruction::SharedRelease(_),
            MirInstruction::SharedMove(_),
            MirInstruction::EndFullExpression(_),
        ]
    )));
    assert!(instructions.windows(7).any(|window| matches!(
        window,
        [
            MirInstruction::SharedAllocate(_),
            MirInstruction::SharedInitialize(_),
            MirInstruction::SharedPublish(_),
            MirInstruction::SharedAdopt(_),
            MirInstruction::SharedRelease(_),
            MirInstruction::SharedMove(_),
            MirInstruction::EndFullExpression(_),
        ]
    )));

    let dump = dump_mir(&program);
    assert!(dump.contains("temporary <temporary>"));
    assert!(dump.contains(": shared class c0"));
    assert!(dump.contains("shared-copy"));
    assert!(dump.contains("shared-release"));
    assert!(dump.contains("shared-move"));
}

#[test]
fn carries_named_and_produced_owners_through_parameters_and_results() {
    let program = lower_text(concat!(
        "class Item { init() {} }\n",
        "fn make() -> shared Item { return new Item(); }\n",
        "fn forward(value: shared Item) -> shared Item { return value; }\n",
        "fn replace(value: shared Item, replacement: shared Item) -> shared Item {\n",
        "  value = replacement;\n",
        "  return value;\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var first: shared Item = make();\n",
        "  var copied: shared Item = forward(first);\n",
        "  var produced: shared Item = forward(make());\n",
        "  var replaced: shared Item = replace(copied, produced);\n",
        "  return 0;\n",
        "}\n",
    ));
    verify_mir(&program).expect("shared call ownership must verify");

    let dump = dump_mir(&program);
    assert!(dump.contains("shared-owner("));
    assert!(dump.contains("return-shared"));
    assert!(dump.contains("shared-result"));
}

#[test]
fn rejects_corrupt_call_handoffs_parameter_cleanup_and_shared_returns() {
    let source = concat!(
        "class Item { init() {} }\n",
        "fn make() -> shared Item { return new Item(); }\n",
        "fn forward(value: shared Item) -> shared Item { return value; }\n",
        "fn main() -> i64 {\n",
        "  var first: shared Item = make();\n",
        "  var second: shared Item = forward(first);\n",
        "  return 0;\n",
        "}\n",
    );
    let program = lower_text(source);

    let mut missing_parameter_cleanup = program.clone();
    missing_parameter_cleanup
        .definitions
        .get_mut_for_test(FunctionId::new(1))
        .unwrap()
        .body
        .blocks[0]
        .instructions
        .retain(|instruction| !matches!(instruction, MirInstruction::SharedRelease(_)));
    assert!(has_error(
        &missing_parameter_cleanup,
        "shared owner remains live on normal return"
    ));

    let mut reused_result = program.clone();
    let main = reused_result
        .definitions
        .get_mut_for_test(FunctionId::new(2))
        .unwrap();
    let first = main
        .storage
        .iter()
        .find(|storage| storage.name == "first")
        .unwrap()
        .id;
    let call = main.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call)
                if call.target == MirCallTarget::Direct(FunctionId::new(1)) =>
            {
                Some(call)
            }
            _ => None,
        })
        .unwrap();
    call.shared_result = Some(first);
    assert!(has_error(
        &reused_result,
        "shared call result storage is already initialized"
    ));

    let mut wrong_return = program;
    let forward = wrong_return
        .definitions
        .get_mut_for_test(FunctionId::new(1))
        .unwrap();
    let parameter = forward.parameters[0];
    forward.body.blocks[0].terminator = Some(MirTerminator::ReturnShared {
        owner: parameter,
        span: forward.span,
    });
    assert!(has_error(
        &wrong_return,
        "shared return must transfer the definition's matching return owner"
    ));
}

#[test]
fn rejects_move_before_release_and_live_full_expression_temporaries() {
    let program = lower_text(concat!(
        "class Item { init() {} }\n",
        "fn main() -> i64 {\n",
        "  var source: shared Item = new Item();\n",
        "  var destination: shared Item = source;\n",
        "  destination = source;\n",
        "  return 0;\n",
        "}\n",
    ));

    let mut early_move = program.clone();
    let instructions = main_instructions_mut(&mut early_move);
    let release = instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::SharedRelease(_)))
        .unwrap();
    let transfer = instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::SharedMove(_)))
        .unwrap();
    instructions.swap(release, transfer);
    assert!(has_error(&early_move, "destination is still live"));

    let mut live_temporary = program;
    main_instructions_mut(&mut live_temporary)
        .retain(|instruction| !matches!(instruction, MirInstruction::SharedMove(_)));
    assert!(has_error(
        &live_temporary,
        "temporary remains live at full-expression boundary"
    ));
}

#[test]
fn fully_released_branch_local_owner_does_not_escape_to_the_join() {
    let program = lower_text(concat!(
        "class Widget { init() {} }\n",
        "fn main() -> i64 {\n",
        "  if (true) { var value: shared Widget = new Widget(); }\n",
        "  return 0;\n",
        "}\n",
    ));
    verify_mir(&program).expect("a completed branch-local lifetime must join cleanly");
}

#[test]
fn rejects_duplicate_adoption_and_release_before_publication() {
    let mut duplicate = exact_owner_program();
    let adopt = main_instructions(&duplicate)
        .iter()
        .find_map(|instruction| match instruction {
            MirInstruction::SharedAdopt(adopt) => Some(adopt.clone()),
            _ => None,
        })
        .unwrap();
    let index = main_instructions(&duplicate)
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::EndFullExpression(_)))
        .unwrap();
    main_instructions_mut(&mut duplicate).insert(index, MirInstruction::SharedAdopt(adopt));
    assert!(has_error(
        &duplicate,
        "requires one published produced owner"
    ));

    let mut early = exact_owner_program();
    let instructions = main_instructions_mut(&mut early);
    let publish = instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::SharedPublish(_)))
        .unwrap();
    let adopt = instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::SharedAdopt(_)))
        .unwrap();
    instructions.swap(publish, adopt);
    assert!(has_error(
        &early,
        "adoption requires one published produced owner"
    ));
}

#[test]
fn rejects_missing_and_duplicate_release() {
    let mut missing = exact_owner_program();
    main_instructions_mut(&mut missing)
        .retain(|instruction| !matches!(instruction, MirInstruction::SharedRelease(_)));
    assert!(has_error(&missing, "remains live on normal return"));

    let mut duplicate = exact_owner_program();
    let release = main_instructions(&duplicate)
        .iter()
        .find_map(|instruction| match instruction {
            MirInstruction::SharedRelease(release) => Some(release.clone()),
            _ => None,
        })
        .unwrap();
    let index = main_instructions(&duplicate)
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::SharedRelease(_)))
        .unwrap();
    main_instructions_mut(&mut duplicate).insert(index + 1, MirInstruction::SharedRelease(release));
    assert!(has_error(&duplicate, "released without being live"));
}

#[test]
fn rejects_wrong_target_and_non_new_allocation() {
    let mut wrong_target = exact_owner_program();
    let owner = wrong_target
        .definitions
        .get_mut_for_test(wrong_target.entry_function)
        .unwrap()
        .storage
        .iter_mut()
        .find(|storage| matches!(storage.ty, MirType::Shared(_)))
        .unwrap();
    owner.ty = MirType::Shared(MirSharedTarget::Class(ClassId::new(99)));
    assert!(has_error(
        &wrong_target,
        "requires compatible exact-class destination owner storage"
    ));

    let mut non_new = exact_owner_program();
    let allocation = main_instructions_mut(&mut non_new)
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::SharedAllocate(allocation) => Some(allocation),
            _ => None,
        })
        .unwrap();
    allocation.origin = MirSharedAllocationOrigin::Unspecified;
    assert!(has_error(&non_new, "does not originate from `new`"));
}

#[test]
fn rejects_use_after_release_and_different_join_states() {
    let mut use_after_release = exact_owner_program();
    let owner = use_after_release
        .definitions
        .get(use_after_release.entry_function)
        .unwrap()
        .storage
        .iter()
        .find(|storage| matches!(storage.ty, MirType::Shared(_)))
        .unwrap()
        .id;
    let release_index = main_instructions(&use_after_release)
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::SharedRelease(_)))
        .unwrap();
    let span = use_after_release.span;
    main_instructions_mut(&mut use_after_release).insert(
        release_index + 1,
        MirInstruction::SharedCopy(MirSharedCopy {
            destination: owner,
            source: owner,
            span,
        }),
    );
    assert!(has_error(
        &use_after_release,
        "copy source is not a live owner"
    ));

    let mut join = exact_owner_program();
    let function = join
        .definitions
        .get_mut_for_test(join.entry_function)
        .unwrap();
    let span = function.span;
    let original = function.body.blocks.pop().unwrap();
    let split = original
        .instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::SharedRelease(_)))
        .unwrap();
    let mut before_release = original.instructions;
    let after_release = before_release.split_off(split);
    let condition = ValueId::new(function.function, function.values.len());
    function
        .values
        .push(fixture_value(condition, MirType::Bool, span));
    let entry = BlockId::new(function.function, 0);
    let released = BlockId::new(function.function, 1);
    let live = BlockId::new(function.function, 2);
    let exit = BlockId::new(function.function, 3);
    before_release.push(fixture_assign(
        condition,
        MirRvalueKind::ConstantBool(true),
        MirType::Bool,
        span,
    ));
    function.body.blocks = vec![
        fixture_block(
            entry,
            before_release,
            Some(MirTerminator::Branch {
                condition,
                true_target: released,
                false_target: live,
                span,
            }),
            span,
        ),
        fixture_block(
            released,
            after_release,
            Some(MirTerminator::Goto { target: exit, span }),
            span,
        ),
        fixture_block(
            live,
            vec![],
            Some(MirTerminator::Goto { target: exit, span }),
            span,
        ),
        fixture_block(exit, vec![], original.terminator, span),
    ];
    assert!(has_error(&join, "state differs across control-flow paths"));
}
