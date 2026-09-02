use crate::{
    identity::{CallableId, ClassId},
    mir::{verify_mir, MirInstruction, MirStorage, MirStorageKind, MirType, StorageId},
    test_support::lower_source_to_mir,
};

fn local_fixture() -> crate::mir::MirProgram {
    lower_source_to_mir("fn main() -> i64 { var value: i64 = 7; return value; }")
}

fn errors_contain(program: &crate::mir::MirProgram, expected: &str) -> bool {
    verify_mir(program)
        .expect_err("mutated lifetime fixture must fail verification")
        .iter()
        .any(|error| error.message.contains(expected))
}

#[test]
fn lowered_local_has_one_balanced_lifetime_epoch() {
    let program = local_fixture();
    verify_mir(&program).expect("lowered local lifetime must verify");
    let function = program.definitions.get(program.entry_function).unwrap();
    let local = function
        .storage
        .iter()
        .find(|storage| {
            storage
                .source
                .is_some_and(|source| matches!(source, crate::identity::BindingId::Local(_)))
        })
        .unwrap()
        .id;
    let operations: Vec<_> = function.body.blocks[0]
        .instructions
        .iter()
        .filter_map(|instruction| match instruction {
            MirInstruction::StorageLive(operation) if operation.storage == local => Some(true),
            MirInstruction::StorageDead(operation) if operation.storage == local => Some(false),
            _ => None,
        })
        .collect();
    assert_eq!(operations, [true, false]);
}

#[test]
fn accepts_repeated_epochs_for_one_static_storage_identity() {
    let mut program = local_fixture();
    let function = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    let block = &mut function.body.blocks[0];
    let live = block
        .instructions
        .iter()
        .find(|instruction| matches!(instruction, MirInstruction::StorageLive(_)))
        .unwrap()
        .clone();
    let dead = block
        .instructions
        .iter()
        .find(|instruction| matches!(instruction, MirInstruction::StorageDead(_)))
        .unwrap()
        .clone();
    let last_dead = block
        .instructions
        .iter()
        .rposition(|instruction| matches!(instruction, MirInstruction::StorageDead(_)))
        .unwrap();
    block
        .instructions
        .splice(last_dead + 1..last_dead + 1, [live, dead]);

    verify_mir(&program).expect("a later epoch of the same static storage must verify");
}

#[test]
fn accepts_an_inert_temporary_declaration_without_a_lifetime_epoch() {
    let mut program = lower_source_to_mir(concat!(
        "class Value { init() {} }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    let function = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    let id = StorageId::new(function.callable(), function.storage.len());
    function.storage.push(MirStorage {
        id,
        source: None,
        name: "removed-unreachable-temporary".to_owned(),
        kind: MirStorageKind::Temporary,
        ty: MirType::Class(ClassId::new(0)),
        span: function.span,
    });

    verify_mir(&program).expect("an unreferenced retained declaration has no dynamic epoch");
}

#[test]
fn rejects_duplicate_live_and_dead_transitions() {
    let mut duplicate_live = local_fixture();
    let function = duplicate_live
        .definitions
        .get_mut_for_test(duplicate_live.entry_function)
        .unwrap();
    let block = &mut function.body.blocks[0];
    let live = block
        .instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::StorageLive(_)))
        .unwrap();
    let operation = block.instructions[live].clone();
    block.instructions.insert(live + 1, operation);
    assert!(errors_contain(&duplicate_live, "is already live"));

    let mut duplicate_dead = local_fixture();
    let function = duplicate_dead
        .definitions
        .get_mut_for_test(duplicate_dead.entry_function)
        .unwrap();
    let block = &mut function.body.blocks[0];
    let dead = block
        .instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::StorageDead(_)))
        .unwrap();
    let operation = block.instructions[dead].clone();
    block.instructions.insert(dead + 1, operation);
    assert!(errors_contain(&duplicate_dead, "is already dead"));
}

#[test]
fn rejects_use_after_dead_and_live_storage_on_return() {
    let mut use_after_dead = local_fixture();
    let function = use_after_dead
        .definitions
        .get_mut_for_test(use_after_dead.entry_function)
        .unwrap();
    let block = &mut function.body.blocks[0];
    let dead = block
        .instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::StorageDead(_)))
        .unwrap();
    let operation = block.instructions.remove(dead);
    let live = block
        .instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::StorageLive(_)))
        .unwrap();
    block.instructions.insert(live + 1, operation);
    assert!(errors_contain(
        &use_after_dead,
        "used outside a live lifetime epoch"
    ));

    let mut leaked = local_fixture();
    let function = leaked
        .definitions
        .get_mut_for_test(leaked.entry_function)
        .unwrap();
    function.body.blocks[0]
        .instructions
        .retain(|instruction| !matches!(instruction, MirInstruction::StorageDead(_)));
    assert!(errors_contain(&leaked, "remains live on normal return"));
}

#[test]
fn rejects_lifetime_operation_for_undeclared_storage() {
    let mut program = local_fixture();
    let function = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    let undeclared = StorageId::new(
        CallableId::Function(function.function),
        function.storage.len() + 1,
    );
    let operation = function.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::StorageLive(operation) => Some(operation),
            _ => None,
        })
        .unwrap();
    operation.storage = undeclared;
    assert!(errors_contain(
        &program,
        "storage-live references undeclared storage"
    ));

    let mut program = local_fixture();
    let function = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    let operation = function.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::StorageDead(operation) => Some(operation),
            _ => None,
        })
        .unwrap();
    operation.storage = undeclared;
    assert!(errors_contain(
        &program,
        "storage-dead references undeclared storage"
    ));
}

#[test]
fn rejects_cleanup_after_storage_is_dead() {
    let mut program = lower_source_to_mir(concat!(
        "class Value { init() {} }\n",
        "fn main() -> i64 { var value: Value = Value(); return 0; }\n",
    ));
    let function = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    let block = &mut function.body.blocks[0];
    let cleanup = block
        .instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::Cleanup(_)))
        .unwrap();
    let dead = block
        .instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::StorageDead(_)))
        .unwrap();
    block.instructions.swap(cleanup, dead);

    assert!(errors_contain(
        &program,
        "used outside a live lifetime epoch"
    ));
}

#[test]
fn accepts_matching_diamond_state_and_rejects_disagreement_at_the_join() {
    let source = concat!(
        "fn main() -> i64 {\n",
        "  var value: i64 = 0;\n",
        "  if (true) { value = 1; } else { value = 2; }\n",
        "  return value;\n",
        "}\n",
    );
    let program = lower_source_to_mir(source);
    verify_mir(&program).expect("matching storage state at a diamond join must verify");

    let mut disagreement = program;
    let function = disagreement
        .definitions
        .get_mut_for_test(disagreement.entry_function)
        .unwrap();
    let local = function
        .storage
        .iter()
        .find(|storage| storage.kind == crate::mir::MirStorageKind::Local)
        .unwrap()
        .id;
    let branch = function
        .body
        .blocks
        .iter_mut()
        .find(|block| {
            block.instructions.iter().any(|instruction| {
                matches!(
                    instruction,
                    MirInstruction::Assign(assignment)
                        if matches!(assignment.rvalue.kind, crate::mir::MirRvalueKind::ConstantI64(1))
                )
            })
        })
        .expect("fixture must have a true branch");
    branch
        .instructions
        .push(MirInstruction::StorageDead(crate::mir::MirStorageDead {
            storage: local,
            span: branch.span,
        }));

    assert!(errors_contain(
        &disagreement,
        "storage lifetime state disagrees at control-flow join"
    ));
}

#[test]
fn implicit_callable_storage_uses_the_documented_entry_convention() {
    let program = lower_source_to_mir(concat!(
        "class Value { init() {} }\n",
        "fn identity(value: Value) -> Value { return value; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    verify_mir(&program).expect("parameters and hidden result storage are implicitly live");
    let identity = program
        .definitions
        .get(crate::identity::FunctionId::new(0))
        .unwrap();
    assert!(identity.storage.iter().any(|storage| {
        matches!(
            storage.kind,
            crate::mir::MirStorageKind::Parameter | crate::mir::MirStorageKind::Return
        )
    }));
    assert!(!identity
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .any(|instruction| matches!(
            instruction,
            MirInstruction::StorageLive(_) | MirInstruction::StorageDead(_)
        )));
    assert_eq!(program.classes.iter().next().unwrap().id, ClassId::new(0));
}
