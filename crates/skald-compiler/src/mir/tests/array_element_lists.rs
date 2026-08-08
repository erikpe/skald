use super::*;

fn primitive_element_list_program() -> MirProgram {
    lower_text(concat!(
        "fn exercise() -> i64 {\n",
        "  var values: i64[] = i64[]{11, 22, 33};\n",
        "  return values[0];\n",
        "}\n",
        "fn main() -> i64 { return exercise(); }\n",
    ))
}

fn element_list_error_after(mutator: impl FnOnce(&mut MirProgram)) -> String {
    let mut program = primitive_element_list_program();
    verify_mir(&program).expect("primitive element-list mutation seed must be valid");
    mutator(&mut program);
    verify_mir(&program)
        .expect_err("malformed primitive element-list MIR must be rejected")
        .to_string()
}

fn class_element_list_program() -> MirProgram {
    lower_text(concat!(
        "class Item {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  copy(ref other: Item) { self.value = other.value + 10; }\n",
        "}\n",
        "fn make(value: i64) -> Item { return Item(value); }\n",
        "fn exercise() -> i64 {\n",
        "  var source: Item = Item(3);\n",
        "  var values: Item[] = Item[]{Item(1), make(2), source, (Item(4))};\n",
        "  return values[0].value;\n",
        "}\n",
        "fn main() -> i64 { return exercise(); }\n",
    ))
}

fn class_element_list_error_after(mutator: impl FnOnce(&mut MirProgram)) -> String {
    let mut program = class_element_list_program();
    verify_mir(&program).expect("class element-list mutation seed must be valid");
    mutator(&mut program);
    verify_mir(&program)
        .expect_err("malformed class element-list MIR must be rejected")
        .to_string()
}

fn optional_element_list_program() -> MirProgram {
    lower_text(concat!(
        "class Item {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  copy(ref other: Item) { self.value = other.value + 10; }\n",
        "}\n",
        "fn make(value: i64) -> Item { return Item(value); }\n",
        "fn maybe(value: i64) -> Item? { return Item(value); }\n",
        "fn exercise() -> i64 {\n",
        "  var source: Item = Item(3);\n",
        "  var optional: Item? = Item(5);\n",
        "  var scalars: i64?[] = i64?[]{none, 1};\n",
        "  var values: Item?[] = Item?[]{none, Item(1), make(2), source, optional, (Item(4)), maybe(6)};\n",
        "  return scalars[1]! + (i64) values.len();\n",
        "}\n",
        "fn main() -> i64 { return exercise(); }\n",
    ))
}

fn optional_element_list_error_after(mutator: impl FnOnce(&mut MirProgram)) -> String {
    let mut program = optional_element_list_program();
    verify_mir(&program).expect("optional element-list mutation seed must be valid");
    mutator(&mut program);
    verify_mir(&program)
        .expect_err("malformed optional element-list MIR must be rejected")
        .to_string()
}

#[test]
fn primitive_element_lists_lower_to_a_linear_initialized_prefix() {
    let program = primitive_element_list_program();
    verify_mir(&program).expect("primitive element-list MIR must verify");
    let function = program.definitions.get(FunctionId::new(0)).unwrap();
    let operations: Vec<_> = function
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match instruction {
            MirInstruction::Array(operation) => Some(operation),
            _ => None,
        })
        .collect();

    assert_eq!(
        operations
            .iter()
            .filter(|operation| matches!(operation, MirArrayInstruction::AllocateElements { .. }))
            .count(),
        1
    );
    assert_eq!(
        operations
            .iter()
            .filter_map(|operation| match operation {
                MirArrayInstruction::InitializeElement { position, .. } => Some(*position),
                _ => None,
            })
            .collect::<Vec<_>>(),
        vec![0, 1, 2]
    );
    assert!(operations
        .iter()
        .any(|operation| matches!(operation, MirArrayInstruction::Publish { .. })));
    assert!(!function
        .body
        .blocks
        .iter()
        .any(|block| matches!(block.terminator, Some(MirTerminator::ArrayLoop { .. }))));
    assert!(!operations.iter().any(|operation| matches!(
        operation,
        MirArrayInstruction::InitializeNext { .. } | MirArrayInstruction::CopyNext { .. }
    )));

    let dump = dump_mir(&program);
    assert!(dump.contains("array-allocate-elements"), "{dump}");
    assert!(dump.contains("array-initialize-element"), "{dump}");
    assert_eq!(dump, dump_mir(&primitive_element_list_program()));
}

#[test]
fn primitive_element_list_allocation_precedes_effects_and_enclosing_boundary() {
    let program = lower_text(concat!(
        "fn record(value: i64) -> i64 { return value; }\n",
        "fn exercise() -> unit {\n",
        "  var values: i64[] = i64[]{record(1), record(2)};\n",
        "  return;\n",
        "}\n",
        "fn main() -> i64 { exercise(); return 0; }\n",
    ));
    let function = program.definitions.get(FunctionId::new(1)).unwrap();
    let allocation_block = function
        .body
        .blocks
        .iter()
        .find(|block| {
            block.instructions.iter().any(|instruction| {
                matches!(
                    instruction,
                    MirInstruction::Array(MirArrayInstruction::AllocateElements { .. })
                )
            })
        })
        .unwrap();
    assert!(!allocation_block
        .instructions
        .iter()
        .any(|instruction| matches!(instruction, MirInstruction::Call(_))));
    let Some(MirTerminator::ArrayOperationCheck { success_target, .. }) =
        allocation_block.terminator
    else {
        panic!("element-list allocation must be followed by its checked failure edge");
    };
    let success = function.block(success_target).unwrap();
    let calls: Vec<_> = success
        .instructions
        .iter()
        .enumerate()
        .filter_map(|(index, instruction)| {
            matches!(instruction, MirInstruction::Call(_)).then_some(index)
        })
        .collect();
    let initialized: Vec<_> = success
        .instructions
        .iter()
        .enumerate()
        .filter_map(|(index, instruction)| match instruction {
            MirInstruction::Array(MirArrayInstruction::InitializeElement { position, .. }) => {
                Some((index, *position))
            }
            _ => None,
        })
        .collect();
    let publication = success
        .instructions
        .iter()
        .position(|instruction| {
            matches!(
                instruction,
                MirInstruction::Array(MirArrayInstruction::Publish { .. })
            )
        })
        .unwrap();
    let boundary = success
        .instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::EndFullExpression(_)))
        .unwrap();

    assert_eq!(calls.len(), 2);
    assert_eq!(initialized.len(), 2);
    assert_eq!((initialized[0].1, initialized[1].1), (0, 1));
    assert!(calls[0] < initialized[0].0);
    assert!(initialized[0].0 < calls[1]);
    assert!(calls[1] < initialized[1].0);
    assert!(initialized[1].0 < publication);
    assert!(publication < boundary);
}

#[test]
fn verifier_rejects_primitive_element_list_prefix_mutations() {
    let errors = element_list_error_after(|program| {
        let function = program
            .definitions
            .get_mut_for_test(FunctionId::new(0))
            .unwrap();
        for block in &mut function.body.blocks {
            block.instructions.retain(|instruction| {
                !matches!(
                    instruction,
                    MirInstruction::Array(MirArrayInstruction::InitializeElement {
                        position: 1,
                        ..
                    })
                )
            });
        }
    });
    assert!(errors.contains("completed unpublished backing"), "{errors}");

    let errors = element_list_error_after(|program| {
        let function = program
            .definitions
            .get_mut_for_test(FunctionId::new(0))
            .unwrap();
        let block = function
            .body
            .blocks
            .iter_mut()
            .find(|block| {
                block.instructions.iter().any(|instruction| {
                    matches!(
                        instruction,
                        MirInstruction::Array(MirArrayInstruction::InitializeElement { .. })
                    )
                })
            })
            .unwrap();
        let first = block
            .instructions
            .iter()
            .position(|instruction| {
                matches!(
                    instruction,
                    MirInstruction::Array(MirArrayInstruction::InitializeElement { .. })
                )
            })
            .unwrap();
        let duplicate = block.instructions[first].clone();
        block.instructions.insert(first + 1, duplicate);
    });
    assert!(errors.contains("source-ordered prefix"), "{errors}");

    let errors = element_list_error_after(|program| {
        let function = program
            .definitions
            .get_mut_for_test(FunctionId::new(0))
            .unwrap();
        for block in &mut function.body.blocks {
            for instruction in &mut block.instructions {
                if let MirInstruction::Array(MirArrayInstruction::InitializeElement {
                    position,
                    ..
                }) = instruction
                {
                    *position = 2;
                    return;
                }
            }
        }
    });
    assert!(errors.contains("source-ordered prefix"), "{errors}");

    let errors = element_list_error_after(|program| {
        let function = program
            .definitions
            .get_mut_for_test(FunctionId::new(0))
            .unwrap();
        let value = function
            .body
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .find_map(|instruction| match instruction {
                MirInstruction::Array(MirArrayInstruction::InitializeElement { value, .. }) => {
                    Some(*value)
                }
                _ => None,
            })
            .unwrap();
        function.values[value.index()].ty = MirType::Bool;
    });
    assert!(errors.contains("exact primitive value"), "{errors}");
}

#[test]
fn verifier_rejects_post_publication_and_duplicated_element_list_backings() {
    let errors = element_list_error_after(|program| {
        let function = program
            .definitions
            .get_mut_for_test(FunctionId::new(0))
            .unwrap();
        let initialization = function
            .body
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .find(|instruction| {
                matches!(
                    instruction,
                    MirInstruction::Array(MirArrayInstruction::InitializeElement { .. })
                )
            })
            .unwrap()
            .clone();
        let block = function
            .body
            .blocks
            .iter_mut()
            .find(|block| {
                block.instructions.iter().any(|instruction| {
                    matches!(
                        instruction,
                        MirInstruction::Array(MirArrayInstruction::Publish { .. })
                    )
                })
            })
            .unwrap();
        let publication = block
            .instructions
            .iter()
            .position(|instruction| {
                matches!(
                    instruction,
                    MirInstruction::Array(MirArrayInstruction::Publish { .. })
                )
            })
            .unwrap();
        block.instructions.insert(publication + 1, initialization);
    });
    assert!(
        errors.contains("live unpublished element-list backing"),
        "{errors}"
    );

    let errors = element_list_error_after(|program| {
        let function = program
            .definitions
            .get_mut_for_test(FunctionId::new(0))
            .unwrap();
        for block in &mut function.body.blocks {
            block.instructions.retain(|instruction| {
                !matches!(
                    instruction,
                    MirInstruction::Array(MirArrayInstruction::Publish { .. })
                )
            });
        }
    });
    assert!(
        errors.contains("owner state must be fully consumed")
            || errors.contains("owner state remains active at storage-dead"),
        "{errors}"
    );

    let errors = element_list_error_after(|program| {
        let function = program
            .definitions
            .get_mut_for_test(FunctionId::new(0))
            .unwrap();
        let block = function
            .body
            .blocks
            .iter_mut()
            .find(|block| {
                block.instructions.iter().any(|instruction| {
                    matches!(
                        instruction,
                        MirInstruction::Array(MirArrayInstruction::AllocateElements { .. })
                    )
                })
            })
            .unwrap();
        let allocation = block
            .instructions
            .iter()
            .position(|instruction| {
                matches!(
                    instruction,
                    MirInstruction::Array(MirArrayInstruction::AllocateElements { .. })
                )
            })
            .unwrap();
        let duplicate = block.instructions[allocation].clone();
        block.instructions.insert(allocation, duplicate);
    });
    assert!(errors.contains("allocated more than once"), "{errors}");
}

#[test]
fn class_element_lists_reuse_destination_directed_object_operations() {
    let program = class_element_list_program();
    verify_mir(&program).expect("class element-list MIR must verify");
    let function = program.definitions.get(FunctionId::new(1)).unwrap();
    let instructions: Vec<_> = function
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .collect();
    let completions = instructions
        .iter()
        .filter_map(|instruction| match instruction {
            MirInstruction::Array(MirArrayInstruction::CompleteElement { position, .. }) => {
                Some(*position)
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(completions, vec![0, 1, 2, 3]);
    assert_eq!(
        instructions
            .iter()
            .filter(|instruction| matches!(instruction, MirInstruction::Initialize(_)))
            .count(),
        3,
        "source local, direct slot, and grouped temporary each initialize once"
    );
    assert!(instructions.iter().any(|instruction| matches!(
        instruction,
        MirInstruction::Call(call)
            if call.destination.as_ref().is_some_and(|place|
                matches!(place.projections.as_slice(), [MirPlaceProjection::ArrayElement { .. }]))
    )));
    assert_eq!(
        instructions
            .iter()
            .filter(|instruction| matches!(instruction, MirInstruction::CopyConstruct(_)))
            .count(),
        2,
        "named and grouped sources must be copy-constructed"
    );
    assert!(instructions.iter().any(|instruction| matches!(
        instruction,
        MirInstruction::EndFullExpression(end) if end.temporaries.len() == 1
    )));
    assert!(!instructions.iter().any(|instruction| matches!(
        instruction,
        MirInstruction::Array(
            MirArrayInstruction::InitializeNext { .. }
                | MirArrayInstruction::CopyNext { .. }
                | MirArrayInstruction::ElementAssign { .. }
        )
    )));

    let dump = dump_mir(&program);
    assert!(dump.contains("array-complete-element"), "{dump}");
    assert!(dump.contains("copy-construct"), "{dump}");
    assert_eq!(dump, dump_mir(&class_element_list_program()));
}

#[test]
fn optional_element_lists_reuse_conditional_initialization_and_payload_destinations() {
    let program = optional_element_list_program();
    verify_mir(&program).expect("optional element-list MIR must verify");
    let function = program.definitions.get(FunctionId::new(2)).unwrap();
    let instructions = function
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .collect::<Vec<_>>();

    assert_eq!(
        instructions
            .iter()
            .filter(|instruction| matches!(instruction, MirInstruction::OptionalInitialize(_)))
            .count(),
        2
    );
    assert!(instructions.iter().any(|instruction| matches!(
        instruction,
        MirInstruction::ClassOptionalInitialize(initialize)
            if matches!(initialize.source, MirClassOptionalSource::Absent)
                && matches!(initialize.destination.projections.as_slice(),
                    [MirPlaceProjection::ArrayElement { .. }])
    )));
    assert!(instructions.iter().any(|instruction| matches!(
        instruction,
        MirInstruction::ClassOptionalPublish(publish)
            if matches!(publish.destination.projections.as_slice(),
                [MirPlaceProjection::ArrayElement { .. }])
    )));
    assert!(instructions.iter().any(|instruction| matches!(
        instruction,
        MirInstruction::Initialize(initialize)
            if matches!(initialize.destination.projections.as_slice(),
                    [MirPlaceProjection::ArrayElement { .. }, MirPlaceProjection::OptionalPayload(_)])
    )));
    assert!(instructions.iter().any(|instruction| matches!(
        instruction,
        MirInstruction::ClassOptionalInitialize(initialize)
            if initialize.copy_constructor.is_some()
                && matches!(initialize.source, MirClassOptionalSource::Present(_))
                && matches!(initialize.destination.projections.as_slice(),
                    [MirPlaceProjection::ArrayElement { .. }])
    )));
    assert!(instructions.iter().any(|instruction| matches!(
        instruction,
        MirInstruction::ClassOptionalInitialize(initialize)
            if initialize.copy_constructor.is_some()
                && matches!(initialize.source, MirClassOptionalSource::Copy(_))
                && matches!(initialize.destination.projections.as_slice(),
                    [MirPlaceProjection::ArrayElement { .. }])
    )));
    assert_eq!(
        instructions
            .iter()
            .filter(|instruction| matches!(
                instruction,
                MirInstruction::Array(MirArrayInstruction::CompleteElement { .. })
            ))
            .count(),
        9
    );

    let dump = dump_mir(&program);
    assert!(dump.contains("optional-initialize"), "{dump}");
    assert!(dump.contains("class-optional-publish"), "{dump}");
    assert_eq!(dump, dump_mir(&optional_element_list_program()));
}

#[test]
fn verifier_rejects_incomplete_optional_element_publication() {
    let errors = optional_element_list_error_after(|program| {
        let function = program
            .definitions
            .get_mut_for_test(FunctionId::new(2))
            .unwrap();
        for block in &mut function.body.blocks {
            if let Some(publication) = block.instructions.iter().position(|instruction| {
                matches!(instruction, MirInstruction::ClassOptionalPublish(publish)
                    if matches!(publish.destination.projections.as_slice(),
                        [MirPlaceProjection::ArrayElement { .. }]))
            }) {
                block.instructions.remove(publication);
                return;
            }
        }
        panic!("fixture must contain class optional publication");
    });
    assert!(
        errors.contains("constructed source-ordered prefix"),
        "{errors}"
    );

    let errors = optional_element_list_error_after(|program| {
        let function = program
            .definitions
            .get_mut_for_test(FunctionId::new(2))
            .unwrap();
        for block in &mut function.body.blocks {
            let Some(initialization) = block.instructions.iter().position(|instruction| {
                matches!(instruction, MirInstruction::OptionalInitialize(_))
            }) else {
                continue;
            };
            let completion = block.instructions[initialization + 1..]
                .iter()
                .position(|instruction| {
                    matches!(
                        instruction,
                        MirInstruction::Array(MirArrayInstruction::CompleteElement { .. })
                    )
                })
                .map(|offset| initialization + 1 + offset)
                .unwrap();
            block.instructions.swap(initialization, completion);
            return;
        }
        panic!("fixture must contain primitive optional initialization");
    });
    assert!(
        errors.contains("constructed source-ordered prefix"),
        "{errors}"
    );
}

#[test]
fn verifier_rejects_class_element_completion_mutations() {
    let errors = class_element_list_error_after(|program| {
        let function = program
            .definitions
            .get_mut_for_test(FunctionId::new(1))
            .unwrap();
        for block in &mut function.body.blocks {
            block.instructions.retain(|instruction| {
                !matches!(
                    instruction,
                    MirInstruction::Array(MirArrayInstruction::CompleteElement { position: 1, .. })
                )
            });
        }
    });
    assert!(
        errors.contains("constructed source-ordered prefix"),
        "{errors}"
    );

    let errors = class_element_list_error_after(|program| {
        let function = program
            .definitions
            .get_mut_for_test(FunctionId::new(1))
            .unwrap();
        let block = function
            .body
            .blocks
            .iter_mut()
            .find(|block| {
                block.instructions.iter().any(|instruction| {
                    matches!(
                        instruction,
                        MirInstruction::Array(MirArrayInstruction::CompleteElement { .. })
                    )
                })
            })
            .unwrap();
        let completion = block
            .instructions
            .iter()
            .position(|instruction| {
                matches!(
                    instruction,
                    MirInstruction::Array(MirArrayInstruction::CompleteElement { .. })
                )
            })
            .unwrap();
        let duplicate = block.instructions[completion].clone();
        block.instructions.insert(completion + 1, duplicate);
    });
    assert!(
        errors.contains("constructed source-ordered prefix"),
        "{errors}"
    );

    let errors = class_element_list_error_after(|program| {
        let function = program
            .definitions
            .get_mut_for_test(FunctionId::new(1))
            .unwrap();
        for block in &mut function.body.blocks {
            let Some(completion) = block.instructions.iter().position(|instruction| {
                matches!(
                    instruction,
                    MirInstruction::Array(MirArrayInstruction::CompleteElement { position: 0, .. })
                )
            }) else {
                continue;
            };
            let initialization = block.instructions[..completion]
                .iter()
                .rposition(|instruction| matches!(instruction, MirInstruction::Initialize(_)))
                .unwrap();
            block.instructions.swap(initialization, completion);
            return;
        }
    });
    assert!(
        errors.contains("constructed source-ordered prefix"),
        "{errors}"
    );

    let errors = class_element_list_error_after(|program| {
        let function = program
            .definitions
            .get_mut_for_test(FunctionId::new(1))
            .unwrap();
        let initialization = function
            .body
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .find(|instruction| {
                matches!(instruction, MirInstruction::Initialize(initialize)
                    if matches!(initialize.destination.projections.as_slice(),
                        [MirPlaceProjection::ArrayElement { .. }]))
            })
            .unwrap()
            .clone();
        let block = function
            .body
            .blocks
            .iter_mut()
            .find(|block| {
                block.instructions.iter().any(|instruction| {
                    matches!(
                        instruction,
                        MirInstruction::Array(MirArrayInstruction::Publish { .. })
                    )
                })
            })
            .unwrap();
        let publication = block
            .instructions
            .iter()
            .position(|instruction| {
                matches!(
                    instruction,
                    MirInstruction::Array(MirArrayInstruction::Publish { .. })
                )
            })
            .unwrap();
        block.instructions.insert(publication + 1, initialization);
    });
    assert!(
        errors.contains("live unpublished element-list backing"),
        "{errors}"
    );
}
