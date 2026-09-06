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

fn class_indexed_program() -> MirProgram {
    lower_text(concat!(
        "class Item {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  copy(ref source: Item) { self.value = source.value; }\n",
        "  fn read() -> i64 { return self.value; }\n",
        "  destroy {}\n",
        "}\n",
        "fn make(value: i64) -> Item { return Item(value); }\n",
        "fn main() -> i64 {\n",
        "  var seed: Item = Item(7);\n",
        "  var direct: Item[] = Item[](2u; index => Item(index));\n",
        "  var copied: Item[] = Item[](2u; index => seed);\n",
        "  var grouped: Item[] = Item[](2u; index => (Item(index)));\n",
        "  var returned: Item[] = Item[](2u; index => make(index));\n",
        "  return direct[1].read() + copied[1].read()\n",
        "    + grouped[1].read() + returned[1].read();\n",
        "}\n",
    ))
}

fn indexed_instruction_position(
    program: &MirProgram,
    function: FunctionId,
    predicate: impl Fn(&MirArrayInstruction) -> bool,
) -> (usize, usize) {
    let function = program.definitions.get(function).unwrap();
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
fn exact_class_indexed_construction_reuses_final_destination_operations() {
    let program = class_indexed_program();
    verify_mir(&program).expect("exact-class indexed construction MIR must verify");
    let main = program.definitions.get(program.entry_function).unwrap();
    let dump = dump_mir(&program);
    let advances: Vec<_> = main
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match instruction {
            MirInstruction::Array(MirArrayInstruction::AdvanceIndexedElement {
                backing,
                prefix,
                ..
            }) => Some((*backing, *prefix)),
            _ => None,
        })
        .collect();

    assert_eq!(advances.len(), 4, "{dump}");
    assert_eq!(dump.matches("array-indexed-advance-complete").count(), 4);
    for (backing, prefix) in advances {
        assert!(main
            .body
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .any(|instruction| match instruction {
                MirInstruction::Initialize(initialize) => {
                    initialize.destination.base == MirPlace::base(backing).base
                        && matches!(
                            initialize.destination.projections.as_slice(),
                            [MirPlaceProjection::ArrayElement { normalized_index, .. }]
                                if *normalized_index == prefix
                        )
                }
                MirInstruction::CopyConstruct(copy) => {
                    copy.destination.base == MirPlace::base(backing).base
                        && matches!(
                            copy.destination.projections.as_slice(),
                            [MirPlaceProjection::ArrayElement { normalized_index, .. }]
                                if *normalized_index == prefix
                        )
                }
                MirInstruction::Call(call) =>
                    call.destination.as_ref().is_some_and(|destination| {
                        destination.base == MirPlace::base(backing).base
                            && matches!(
                                destination.projections.as_slice(),
                                [MirPlaceProjection::ArrayElement { normalized_index, .. }]
                                    if *normalized_index == prefix
                            )
                    }),
                _ => false,
            }));
    }
    assert!(!dump.contains("array-initialize-next"), "{dump}");
}

#[test]
fn verifier_rejects_class_prefix_advance_before_destination_completion() {
    let mut program = lower_text(concat!(
        "class Item { value: i64; init(value: i64) { self.value = value; } }\n",
        "fn main() -> i64 {\n",
        "  var values: Item[] = Item[](2u; index => Item(index));\n",
        "  return values[1].value;\n",
        "}\n",
    ));
    verify_mir(&program).expect("baseline class indexed MIR must verify");
    let entry = program.entry_function;
    let (block, advance) = indexed_instruction_position(&program, entry, |operation| {
        matches!(operation, MirArrayInstruction::AdvanceIndexedElement { .. })
    });
    let function = program.definitions.get_mut_for_test(entry).unwrap();
    let completion = function.body.blocks[block].instructions[..advance]
        .iter()
        .rposition(|instruction| matches!(instruction, MirInstruction::Initialize(_)))
        .unwrap();
    function.body.blocks[block]
        .instructions
        .swap(completion, advance);

    assert_rejected(
        &program,
        "indexed array prefix may advance only after the current lifecycle-bearing slot is complete",
    );
}

#[test]
fn verifier_rejects_class_slot_and_advance_mutations() {
    let original = lower_text(concat!(
        "class Item { value: i64; init(value: i64) { self.value = value; } }\n",
        "fn main() -> i64 {\n",
        "  var values: Item[] = Item[](2u; index => Item(index));\n",
        "  return values[1].value;\n",
        "}\n",
    ));
    verify_mir(&original).expect("baseline class indexed MIR must verify");
    let entry = original.entry_function;

    let mut duplicate_advance = original.clone();
    let (block, advance) = indexed_instruction_position(&duplicate_advance, entry, |operation| {
        matches!(operation, MirArrayInstruction::AdvanceIndexedElement { .. })
    });
    let cloned = duplicate_advance
        .definitions
        .get(entry)
        .unwrap()
        .body
        .blocks[block]
        .instructions[advance]
        .clone();
    duplicate_advance
        .definitions
        .get_mut_for_test(entry)
        .unwrap()
        .body
        .blocks[block]
        .instructions
        .insert(advance + 1, cloned);
    assert_rejected(
        &duplicate_advance,
        "advance its prefix and return to the loop header",
    );

    let mut wrong_slot = original;
    let (block, advance) = indexed_instruction_position(&wrong_slot, entry, |operation| {
        matches!(operation, MirArrayInstruction::AdvanceIndexedElement { .. })
    });
    let prefix = match wrong_slot.definitions.get(entry).unwrap().body.blocks[block].instructions
        [advance]
    {
        MirInstruction::Array(MirArrayInstruction::AdvanceIndexedElement { prefix, .. }) => prefix,
        _ => unreachable!(),
    };
    let foreign_position = wrong_slot
        .definitions
        .get(entry)
        .unwrap()
        .storage
        .iter()
        .find(|storage| storage.kind == MirStorageKind::ArrayPosition && storage.id != prefix)
        .unwrap()
        .id;
    let function = wrong_slot.definitions.get_mut_for_test(entry).unwrap();
    let initialize = function.body.blocks[block]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Initialize(initialize) => Some(initialize),
            _ => None,
        })
        .unwrap();
    let MirPlaceProjection::ArrayElement {
        normalized_index, ..
    } = &mut initialize.destination.projections[0]
    else {
        panic!("class destination must be the current array slot");
    };
    *normalized_index = foreign_position;
    assert_rejected(
        &wrong_slot,
        "indexed class construction must complete exactly once in the current prefix slot",
    );
}

#[test]
fn verifier_rejects_missing_duplicate_and_out_of_order_slot_initialization() {
    let original = primitive_indexed_program();

    let mut missing = original.clone();
    let (block, instruction) =
        indexed_instruction_position(&missing, FunctionId::new(1), |operation| {
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
    let (block, instruction) =
        indexed_instruction_position(&duplicate, FunctionId::new(1), |operation| {
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
    let (block, initialization) =
        indexed_instruction_position(&early, FunctionId::new(1), |operation| {
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
    let (block, instruction) =
        indexed_instruction_position(&incomplete, FunctionId::new(1), |operation| {
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
    let (block, instruction) =
        indexed_instruction_position(&leaked, FunctionId::new(1), |operation| {
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
    let (block, instruction) =
        indexed_instruction_position(&duplicated, FunctionId::new(1), |operation| {
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
