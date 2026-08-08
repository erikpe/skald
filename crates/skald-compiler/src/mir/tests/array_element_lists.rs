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
