use super::*;

fn shared_copy_allocation_program() -> MirProgram {
    lower_text(concat!(
        "class Value {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  copy(ref source: Value) { self.value = source.value + 1; }\n",
        "  fn read() -> i64 { return self.value; }\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var source: Value = Value(40);\n",
        "  var owner: shared Value = new Value(copy source);\n",
        "  return owner->read();\n",
        "}\n",
    ))
}

fn checked_shared_copy_allocation_program() -> MirProgram {
    lower_text(concat!(
        "class Value {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  copy(ref source: Value) { self.value = source.value; }\n",
        "}\n",
        "class Other { init() {} }\n",
        "fn allocate_copy(erased: shared Obj) -> i64 {\n",
        "  var owner: shared Value = new Value(copy *erased);\n",
        "  return 0;\n",
        "}\n",
        "fn main() -> i64 { return allocate_copy(new Value(40)); }\n",
    ))
}

#[test]
fn lowers_copy_allocation_as_source_allocate_copy_publish_adopt() {
    let program = shared_copy_allocation_program();
    verify_mir(&program).expect("copy-allocation MIR must verify");
    let instructions = main_instructions(&program);
    let allocation = instructions
        .iter()
        .position(|instruction| {
            matches!(
                instruction,
                MirInstruction::SharedAllocate(MirSharedAllocate {
                    mode: MirSharedAllocationMode::Copy { .. },
                    ..
                })
            )
        })
        .expect("copy allocation must be explicit in MIR");
    assert!(matches!(
        &instructions[allocation..allocation + 5],
        [
            MirInstruction::SharedAllocate(_),
            MirInstruction::CopyConstruct(_),
            MirInstruction::SharedPublish(_),
            MirInstruction::SharedAdopt(_),
            MirInstruction::EndFullExpression(_),
        ]
    ));
    let MirInstruction::SharedAllocate(allocation_instruction) = &instructions[allocation] else {
        unreachable!();
    };
    let MirInstruction::CopyConstruct(copy) = &instructions[allocation + 1] else {
        unreachable!();
    };
    assert_eq!(allocation_instruction.class, copy.class);
    assert_eq!(
        allocation_instruction.mode,
        MirSharedAllocationMode::Copy {
            source: copy.source.clone()
        }
    );
    assert_eq!(
        copy.destination,
        MirPlace::shared_allocation_payload(allocation_instruction.allocation)
    );
}

#[test]
fn copy_allocation_verifier_rejects_missing_copy_and_precheck_allocation() {
    let mut missing_copy = shared_copy_allocation_program();
    for block in &mut missing_copy
        .definitions
        .get_mut_for_test(missing_copy.entry_function)
        .unwrap()
        .body
        .blocks
    {
        block
            .instructions
            .retain(|instruction| !matches!(instruction, MirInstruction::CopyConstruct(_)));
    }
    assert!(has_error(
        &missing_copy,
        "publication requires completed initialization"
    ));

    let mut before_check = checked_shared_copy_allocation_program();
    let before_check_dump = dump_mir(&before_check);
    let definition = before_check
        .definitions
        .get_mut_for_test(crate::identity::FunctionId::new(0))
        .unwrap();
    let success = definition
        .body
        .blocks
        .iter()
        .position(|block| {
            block.instructions.iter().any(|instruction| {
                matches!(
                    instruction,
                    MirInstruction::SharedAllocate(MirSharedAllocate {
                        mode: MirSharedAllocationMode::Copy { .. },
                        ..
                    })
                )
            })
        })
        .unwrap();
    let allocation_index = definition.body.blocks[success]
        .instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::SharedAllocate(_)))
        .unwrap();
    let allocation = definition.body.blocks[success]
        .instructions
        .remove(allocation_index);
    let predecessor = definition
        .body
        .blocks
        .iter()
        .position(|block| matches!(block.terminator, Some(MirTerminator::CheckedCast { .. })))
        .unwrap_or_else(|| panic!("expected checked-cast predecessor:\n{before_check_dump}"));
    definition.body.blocks[predecessor]
        .instructions
        .push(allocation);
    assert!(has_error(
        &before_check,
        "shared copy-allocation source is not live"
    ));
}
