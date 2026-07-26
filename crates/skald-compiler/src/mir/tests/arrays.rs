use super::*;

fn array_program() -> MirProgram {
    lower_text(concat!(
        "fn exercise() -> i64 {\n",
        "  var values: i64[] = i64[](2u);\n",
        "  values[0] = 7;\n",
        "  return values[0];\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ))
}

fn error_after(mutator: impl FnOnce(&mut MirProgram)) -> String {
    let mut program = array_program();
    verify_mir(&program).expect("mutation seed must be valid");
    mutator(&mut program);
    verify_mir(&program)
        .expect_err("malformed array MIR must be rejected")
        .to_string()
}

#[test]
fn array_mir_dump_is_deterministic_and_target_independent() {
    let first = dump_mir(&array_program());
    let second = dump_mir(&array_program());
    assert_eq!(first, second);
    for expected in [
        "ArrayTypes",
        "array-backing",
        "array-produced",
        "array-anchor",
        "array-loop",
        "array-operation-check",
        "array-position-check",
    ] {
        assert!(first.contains(expected), "missing {expected}:\n{first}");
    }
    for forbidden in ["stride=", "offset=", "header-bytes", "x86"] {
        assert!(!first.contains(forbidden));
    }
}

#[test]
fn class_array_elements_participate_in_copy_assignment() {
    let program = lower_text(concat!(
        "class Item {\n",
        "  value: i64;\n",
        "  init() { self.value = 0; }\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var source: Item[] = Item[](1u);\n",
        "  var destination: Item[] = Item[](1u);\n",
        "  source[0].value = 7;\n",
        "  destination[0] = source[0];\n",
        "  return destination[0].value;\n",
        "}\n",
    ));

    verify_mir(&program).expect("checked array elements must have dynamic object liveness");
}

#[test]
fn verifier_rejects_array_table_type_storage_prefix_and_publication_mutations() {
    let errors = error_after(|program| {
        program.array_types.entries_mut_for_test()[0].id = crate::identity::ArrayTypeId::new(7);
    });
    assert!(errors.contains("array type table index"));

    let errors = error_after(|program| {
        program.array_types.entries_mut_for_test()[0].element =
            MirType::OptionalPrimitive(MirPrimitiveType::I64);
    });
    assert!(errors.contains("lifecycle is incompatible"));

    let errors = error_after(|program| {
        let function = program
            .definitions
            .get_mut_for_test(FunctionId::new(0))
            .unwrap();
        let backing = function
            .storage
            .iter_mut()
            .find(|storage| storage.kind == MirStorageKind::ArrayBacking)
            .unwrap();
        backing.kind = MirStorageKind::Local;
    });
    assert!(errors.contains("unpublished backing storage"));

    let errors = error_after(|program| {
        let function = program
            .definitions
            .get_mut_for_test(FunctionId::new(0))
            .unwrap();
        for block in &mut function.body.blocks {
            for instruction in &mut block.instructions {
                if let MirInstruction::Array(MirArrayInstruction::InitializeNext {
                    operation,
                    ..
                }) = instruction
                {
                    *operation = MirArrayDefaultElement::OptionalAbsent;
                    return;
                }
            }
        }
    });
    assert!(errors.contains("declared element lifecycle"));

    let errors = error_after(|program| {
        let function = program
            .definitions
            .get_mut_for_test(FunctionId::new(0))
            .unwrap();
        let produced = function
            .storage
            .iter()
            .find(|storage| storage.kind == MirStorageKind::ArrayProduced)
            .unwrap()
            .id;
        let backing = function
            .body
            .blocks
            .iter_mut()
            .find_map(|block| match block.terminator.as_mut() {
                Some(MirTerminator::ArrayLoop { backing, .. }) => Some(backing),
                _ => None,
            })
            .unwrap();
        *backing = produced;
    });
    assert!(
        errors.contains("unpublished backing")
            || errors.contains("prefix")
            || errors.contains("owner state")
    );

    let errors = error_after(|program| {
        let function = program
            .definitions
            .get_mut_for_test(FunctionId::new(0))
            .unwrap();
        let foreign = function
            .storage
            .iter()
            .find(|storage| storage.kind == MirStorageKind::ArrayPosition)
            .unwrap()
            .id;
        for block in &mut function.body.blocks {
            for instruction in &mut block.instructions {
                if let MirInstruction::Array(MirArrayInstruction::Publish { backing, .. }) =
                    instruction
                {
                    *backing = foreign;
                    return;
                }
            }
        }
    });
    assert!(errors.contains("publication requires matching"));
}

#[test]
fn verifier_rejects_array_consumption_projection_failure_and_anchor_mutations() {
    let errors = error_after(|program| {
        let function = program
            .definitions
            .get_mut_for_test(FunctionId::new(0))
            .unwrap();
        for block in &mut function.body.blocks {
            block.instructions.retain(|instruction| {
                !matches!(
                    instruction,
                    MirInstruction::Array(MirArrayInstruction::Adopt { .. })
                )
            });
        }
    });
    assert!(errors.contains("never consumed"));

    let errors = error_after(|program| {
        let function = program
            .definitions
            .get_mut_for_test(FunctionId::new(0))
            .unwrap();
        let position = function
            .storage
            .iter_mut()
            .find(|storage| storage.kind == MirStorageKind::ArrayPosition)
            .unwrap();
        position.kind = MirStorageKind::ScalarSpill;
    });
    assert!(
        errors.contains("array-position storage")
            || errors.contains("position storage")
            || errors.contains("array loop index")
            || errors.contains("prefix operation")
    );

    let errors = error_after(|program| {
        let function = program
            .definitions
            .get_mut_for_test(FunctionId::new(0))
            .unwrap();
        let failure = function
            .body
            .blocks
            .iter()
            .find_map(|block| match block.terminator {
                Some(MirTerminator::ArrayPositionCheck { failure_target, .. }) => {
                    Some(failure_target)
                }
                _ => None,
            })
            .unwrap();
        function.body.blocks[failure.index()].terminator = Some(MirTerminator::Terminate {
            reason: MirTerminationReason::ArraySliceLengthMismatch,
            span: function.span,
        });
    });
    assert!(errors.contains("failure edge must terminate"));

    let errors = error_after(|program| {
        let function = program
            .definitions
            .get_mut_for_test(FunctionId::new(0))
            .unwrap();
        let failure = function
            .body
            .blocks
            .iter()
            .find_map(|block| match block.terminator {
                Some(MirTerminator::ArrayOperationCheck { failure_target, .. }) => {
                    Some(failure_target)
                }
                _ => None,
            })
            .unwrap();
        function.body.blocks[failure.index()].terminator = Some(MirTerminator::Terminate {
            reason: MirTerminationReason::ArraySliceLengthMismatch,
            span: function.span,
        });
    });
    assert!(errors.contains("operation failure edge must terminate"));

    let errors = error_after(|program| {
        let function = program
            .definitions
            .get_mut_for_test(FunctionId::new(0))
            .unwrap();
        let (success, failure) = function
            .body
            .blocks
            .iter()
            .find_map(|block| match block.terminator {
                Some(MirTerminator::ArrayOperationCheck {
                    failure: MirArrayFailure::AllocationSize,
                    success_target,
                    failure_target,
                    ..
                }) => Some((success_target, failure_target)),
                _ => None,
            })
            .unwrap();
        function.body.blocks[failure.index()].terminator = Some(MirTerminator::Goto {
            target: success,
            span: function.span,
        });
    });
    assert!(errors.contains("owner state disagrees at control-flow join"));

    let errors = error_after(|program| {
        let function = program
            .definitions
            .get_mut_for_test(FunctionId::new(0))
            .unwrap();
        for block in &mut function.body.blocks {
            block.instructions.retain(|instruction| {
                !matches!(
                    instruction,
                    MirInstruction::Array(MirArrayInstruction::AnchorEnd { .. })
                )
            });
        }
    });
    assert!(errors.contains("anchor") && errors.contains("not ended"));
}

#[test]
fn verifier_rejects_slice_write_before_length_check() {
    let mut program = lower_text(concat!(
        "fn assign() -> unit {\n",
        "  var left: i64[] = i64[](2u);\n",
        "  var right: i64[] = i64[](2u);\n",
        "  left[:] = right;\n",
        "  return;\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    verify_mir(&program).expect("slice seed must verify");
    let function = program
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    for block in &mut function.body.blocks {
        block.instructions.retain(|instruction| {
            !matches!(
                instruction,
                MirInstruction::Array(MirArrayInstruction::SliceLengthCheck { .. })
            )
        });
    }
    let errors = verify_mir(&program).unwrap_err().to_string();
    assert!(errors.contains("writes before its length check"));
}
