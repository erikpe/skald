use super::*;

fn array_program() -> MirProgram {
    lower_text(concat!(
        "fn exercise() -> i64 {\n",
        "  var maybe: i64? = none;\n",
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
fn private_initializer_array_default_plans_reject_identity_mutations() {
    let mut program = lower_text(concat!(
        "class Item {\n",
        "  private init() {}\n",
        "  static fn arrays() -> unit {\n",
        "    var inline: Item[] = Item[](1u);\n",
        "    var shared: (shared Item)[] = (shared Item)[](1u);\n",
        "    return;\n",
        "  }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    let initializer = InitializerId::new(ClassId::new(0), 0);
    assert_eq!(
        program
            .array_types
            .iter()
            .filter_map(|array| array.lifecycle.default)
            .filter(|default| {
                matches!(
                    default,
                    MirArrayDefaultElement::Class {
                        initializer: selected,
                        ..
                    } | MirArrayDefaultElement::SharedClass {
                        initializer: selected,
                        ..
                    } if *selected == initializer
                )
            })
            .count(),
        2
    );
    verify_mir(&program).expect("authorized private initializer plans must verify");

    let default = program
        .array_types
        .entries_mut_for_test()
        .iter_mut()
        .find_map(|array| array.lifecycle.default.as_mut())
        .expect("fixture must contain a class default plan");
    match default {
        MirArrayDefaultElement::Class {
            initializer: selected,
            ..
        }
        | MirArrayDefaultElement::SharedClass {
            initializer: selected,
            ..
        } => *selected = InitializerId::new(ClassId::new(0), 99),
        _ => panic!("fixture must select a class initializer"),
    }
    let errors = verify_mir(&program)
        .expect_err("mutated initializer identity must be rejected")
        .to_string();
    assert!(
        errors.contains("array default element names an invalid initializer"),
        "{errors}"
    );
}

#[test]
fn optional_box_array_defaults_reject_target_mutations() {
    let mut program = lower_text(concat!(
        "fn main() -> i64 {\n",
        "  var numbers: (shared i64?)[] = (shared i64?)[](1u);\n",
        "  var flags: (shared bool?)[] = (shared bool?)[](1u);\n",
        "  return 0;\n",
        "}\n",
    ));
    verify_mir(&program).expect("typed optional-box array defaults must verify");

    let targets: Vec<_> = program
        .array_types
        .iter()
        .filter_map(|array| match array.lifecycle.default {
            Some(MirArrayDefaultElement::SharedOptionalBoxAbsent(target)) => Some(target),
            _ => None,
        })
        .collect();
    assert_eq!(targets.len(), 2);
    let array = program
        .array_types
        .entries_mut_for_test()
        .iter_mut()
        .find(|array| {
            array.lifecycle.default
                == Some(MirArrayDefaultElement::SharedOptionalBoxAbsent(targets[0]))
        })
        .unwrap();
    array.lifecycle.default = Some(MirArrayDefaultElement::SharedOptionalBoxAbsent(targets[1]));

    let errors = verify_mir(&program)
        .expect_err("a foreign exact box default must be rejected")
        .to_string();
    assert!(
        errors.contains("does not match its declared element type"),
        "{errors}"
    );
}

#[test]
fn verifier_rejects_array_table_type_storage_prefix_and_publication_mutations() {
    let errors = error_after(|program| {
        program.array_types.entries_mut_for_test()[0].id = crate::identity::ArrayTypeId::new(7);
    });
    assert!(errors.contains("array type table index"));

    let errors = error_after(|program| {
        let optional = program.optional_for_payload(MirType::I64).unwrap();
        program.array_types.entries_mut_for_test()[0].element = MirType::Optional(optional);
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

#[test]
fn verifier_rejects_unbound_early_and_incompatible_array_alias_dependencies() {
    let alias_program = || {
        lower_text(concat!(
            "class Item { value: i64; init() { self.value = 0; } }\n",
            "fn read(ref item: Item) -> i64 { return item.value; }\n",
            "fn exercise() -> i64 {\n",
            "  var items: Item[] = Item[](1u);\n",
            "  return read(items[0]);\n",
            "}\n",
            "fn main() -> i64 { return 0; }\n",
        ))
    };

    let mut program = alias_program();
    let function = program
        .definitions
        .get_mut_for_test(FunctionId::new(1))
        .unwrap();
    for block in &mut function.body.blocks {
        block.instructions.retain(|instruction| {
            !matches!(
                instruction,
                MirInstruction::Array(MirArrayInstruction::AliasBind { .. })
            )
        });
    }
    let errors = verify_mir(&program).unwrap_err().to_string();
    assert!(errors.contains("compatible live") || errors.contains("alias"));

    let mut program = alias_program();
    let function = program
        .definitions
        .get_mut_for_test(FunctionId::new(1))
        .unwrap();
    let alias = function
        .storage
        .iter_mut()
        .find(|storage| matches!(storage.kind, MirStorageKind::ArrayAlias(_)))
        .unwrap();
    alias.kind = MirStorageKind::ScalarSpill;
    let errors = verify_mir(&program).unwrap_err().to_string();
    assert!(errors.contains("array alias binding has incompatible carrier"));

    let mut program = lower_text(concat!(
        "fn read(ref values: i64[]) -> u64 { return values.len(); }\n",
        "fn exercise() -> u64 {\n",
        "  var values: i64[] = i64[](1u);\n",
        "  return read(values);\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    let function = program
        .definitions
        .get_mut_for_test(FunctionId::new(1))
        .unwrap();
    let block = function
        .body
        .blocks
        .iter_mut()
        .find(|block| {
            block
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, MirInstruction::Call(_)))
        })
        .unwrap();
    let end = block
        .instructions
        .iter()
        .position(|instruction| {
            matches!(
                instruction,
                MirInstruction::Array(MirArrayInstruction::AnchorEnd { .. })
            )
        })
        .unwrap();
    let end = block.instructions.remove(end);
    let call = block
        .instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::Call(_)))
        .unwrap();
    block.instructions.insert(call, end);
    let errors = verify_mir(&program).unwrap_err().to_string();
    assert!(errors.contains("compatible live"));
}
