use super::*;

fn primitive_indexed_program() -> MirProgram {
    lower_text(concat!(
        "fn record(value: i64) -> i64 { return value; }\n",
        "fn exercise(length: u64) -> i64 {\n",
        "  var values: i64[] = i64[](length; index => record(index * 2));\n",
        "  return values[(i64) length - 1];\n",
        "}\n",
        "fn main() -> i64 { return exercise(3u); }\n",
    ))
}

fn indexed_instruction_position(
    program: &MirProgram,
    predicate: impl Fn(&MirArrayInstruction) -> bool,
) -> (usize, usize) {
    let function = program.definitions.get(FunctionId::new(1)).unwrap();
    function
        .body
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block_index, block)| {
            block
                .instructions
                .iter()
                .position(|instruction| {
                    matches!(instruction, MirInstruction::Array(operation) if predicate(operation))
                })
                .map(|instruction_index| (block_index, instruction_index))
        })
        .expect("indexed protocol instruction must exist")
}

fn assert_rejected(program: &MirProgram, expected: &str) {
    let errors = verify_mir(program).unwrap_err().to_string();
    assert!(errors.contains(expected), "missing `{expected}`:\n{errors}");
}

#[test]
fn primitive_indexed_construction_lowers_to_one_dynamic_prefix_loop() {
    let program = primitive_indexed_program();
    verify_mir(&program).expect("primitive indexed construction MIR must verify");
    let function = program.definitions.get(FunctionId::new(1)).unwrap();
    let dump = dump_mir(&program);

    for operation in [
        "array-indexed-begin",
        "array-loop Indexed",
        "array-indexed-bind",
        "array-indexed-initialize",
        "array-indexed-end",
        "array-indexed-complete",
        "array-publish",
    ] {
        assert!(dump.contains(operation), "missing `{operation}`:\n{dump}");
    }
    assert_eq!(
        function
            .body
            .blocks
            .iter()
            .filter(|block| matches!(
                block.terminator,
                Some(MirTerminator::ArrayLoop {
                    kind: MirArrayLoopKind::Indexed { .. },
                    ..
                })
            ))
            .count(),
        1
    );
}

#[test]
fn indexed_element_temporaries_are_cleaned_inside_each_epoch() {
    let program = lower_text(concat!(
        "class Probe {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  fn read() -> i64 { return self.value; }\n",
        "  destroy {}\n",
        "}\n",
        "fn make(value: i64) -> Probe { return Probe(value); }\n",
        "fn main() -> i64 {\n",
        "  var values: i64[] = i64[](2u; index => make(index).read());\n",
        "  return values[1];\n",
        "}\n",
    ));
    verify_mir(&program).expect("per-index temporary cleanup MIR must verify");
    let main = program.definitions.get(program.entry_function).unwrap();
    let body = main
        .body
        .blocks
        .iter()
        .find(|block| {
            block.instructions.iter().any(|instruction| {
                matches!(
                    instruction,
                    MirInstruction::Array(MirArrayInstruction::EndIndexedElement { .. })
                )
            })
        })
        .unwrap();
    let initialized = body
        .instructions
        .iter()
        .position(|instruction| {
            matches!(
                instruction,
                MirInstruction::Array(MirArrayInstruction::InitializeIndexedElement { .. })
            )
        })
        .unwrap();
    let ended = body
        .instructions
        .iter()
        .position(|instruction| {
            matches!(
                instruction,
                MirInstruction::Array(MirArrayInstruction::EndIndexedElement { .. })
            )
        })
        .unwrap();

    assert!(body.instructions[initialized + 1..ended]
        .iter()
        .any(|instruction| matches!(
            instruction,
            MirInstruction::EndFullExpression(end) if !end.temporaries.is_empty()
        )));
}

#[test]
fn verifier_rejects_missing_duplicate_and_out_of_order_slot_initialization() {
    let original = primitive_indexed_program();

    let mut missing = original.clone();
    let (block, instruction) = indexed_instruction_position(&missing, |operation| {
        matches!(
            operation,
            MirArrayInstruction::InitializeIndexedElement { .. }
        )
    });
    missing
        .definitions
        .get_mut_for_test(FunctionId::new(1))
        .unwrap()
        .body
        .blocks[block]
        .instructions
        .remove(instruction);
    assert_rejected(&missing, "advance its prefix and return to the loop header");

    let mut duplicate = original.clone();
    let (block, instruction) = indexed_instruction_position(&duplicate, |operation| {
        matches!(
            operation,
            MirArrayInstruction::InitializeIndexedElement { .. }
        )
    });
    let cloned = duplicate
        .definitions
        .get(FunctionId::new(1))
        .unwrap()
        .body
        .blocks[block]
        .instructions[instruction]
        .clone();
    duplicate
        .definitions
        .get_mut_for_test(FunctionId::new(1))
        .unwrap()
        .body
        .blocks[block]
        .instructions
        .insert(instruction + 1, cloned);
    assert_rejected(
        &duplicate,
        "advance its prefix and return to the loop header",
    );

    let mut early = original;
    let (block, initialization) = indexed_instruction_position(&early, |operation| {
        matches!(
            operation,
            MirArrayInstruction::InitializeIndexedElement { .. }
        )
    });
    let binding = early
        .definitions
        .get(FunctionId::new(1))
        .unwrap()
        .body
        .blocks[block]
        .instructions
        .iter()
        .position(|instruction| {
            matches!(
                instruction,
                MirInstruction::Array(MirArrayInstruction::BindIndexed { .. })
            )
        })
        .unwrap();
    let initialization = early
        .definitions
        .get_mut_for_test(FunctionId::new(1))
        .unwrap()
        .body
        .blocks[block]
        .instructions
        .remove(initialization);
    early
        .definitions
        .get_mut_for_test(FunctionId::new(1))
        .unwrap()
        .body
        .blocks[block]
        .instructions
        .insert(binding, initialization);
    assert_rejected(
        &early,
        "indexed array element must initialize and advance the exact current slot once",
    );
}

#[test]
fn verifier_rejects_unclosed_epochs_and_invalid_backedges() {
    let original = primitive_indexed_program();

    let mut leaked_binding = original.clone();
    let function = leaked_binding
        .definitions
        .get_mut_for_test(FunctionId::new(1))
        .unwrap();
    let body = function
        .body
        .blocks
        .iter_mut()
        .find(|block| {
            block.instructions.iter().any(|instruction| {
                matches!(
                    instruction,
                    MirInstruction::Array(MirArrayInstruction::EndIndexedElement { .. })
                )
            })
        })
        .unwrap();
    let dead = body
        .instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::StorageDead(_)))
        .unwrap();
    body.instructions.remove(dead);
    assert_rejected(
        &leaked_binding,
        "advance its prefix and return to the loop header",
    );

    let mut invalid_backedge = original;
    let function = invalid_backedge
        .definitions
        .get_mut_for_test(FunctionId::new(1))
        .unwrap();
    let (body_target, complete_target) = function
        .body
        .blocks
        .iter()
        .find_map(|block| match block.terminator {
            Some(MirTerminator::ArrayLoop {
                kind: MirArrayLoopKind::Indexed { .. },
                body_target,
                complete_target,
                ..
            }) => Some((body_target, complete_target)),
            _ => None,
        })
        .unwrap();
    let span = function.body.blocks[body_target.index()].span;
    function.body.blocks[body_target.index()].terminator = Some(MirTerminator::Goto {
        target: complete_target,
        span,
    });
    assert_rejected(
        &invalid_backedge,
        "advance its prefix and return to the loop header",
    );
}

#[test]
fn verifier_rejects_incomplete_and_duplicate_backing_publication() {
    let original = primitive_indexed_program();

    let mut incomplete = original.clone();
    let (block, instruction) = indexed_instruction_position(&incomplete, |operation| {
        matches!(operation, MirArrayInstruction::CompleteIndexed { .. })
    });
    incomplete
        .definitions
        .get_mut_for_test(FunctionId::new(1))
        .unwrap()
        .body
        .blocks[block]
        .instructions
        .remove(instruction);
    assert_rejected(
        &incomplete,
        "advance its prefix and return to the loop header",
    );

    let mut leaked = original.clone();
    let (block, instruction) = indexed_instruction_position(&leaked, |operation| {
        matches!(operation, MirArrayInstruction::Publish { .. })
    });
    leaked
        .definitions
        .get_mut_for_test(FunctionId::new(1))
        .unwrap()
        .body
        .blocks[block]
        .instructions
        .remove(instruction);
    assert_rejected(
        &leaked,
        "produced array storage must be consumed exactly once",
    );

    let mut duplicated = original;
    let (block, instruction) = indexed_instruction_position(&duplicated, |operation| {
        matches!(operation, MirArrayInstruction::Publish { .. })
    });
    let cloned = duplicated
        .definitions
        .get(FunctionId::new(1))
        .unwrap()
        .body
        .blocks[block]
        .instructions[instruction]
        .clone();
    duplicated
        .definitions
        .get_mut_for_test(FunctionId::new(1))
        .unwrap()
        .body
        .blocks[block]
        .instructions
        .insert(instruction + 1, cloned);
    assert_rejected(
        &duplicated,
        "array publication requires one completed unpublished backing",
    );
}
