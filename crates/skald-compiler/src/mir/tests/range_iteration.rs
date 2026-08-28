//! Scalar shape and verifier hardening for fused primitive range iteration.

use crate::{
    test_support::{load_module_sources_with_standard_library, lower_hir_to_final_mir},
    typeck::type_check,
};

use super::*;

fn lowered(source: &str) -> MirProgram {
    let (_workspace, graph) =
        load_module_sources_with_standard_library("app", &[("app.ska", source)]);
    let resolved = crate::resolve::resolve_module_graph(&graph);
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );
    let checked = type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    lower_hir_to_final_mir(&checked.hir.expect("range fixture must produce HIR"))
}

fn function_id(program: &MirProgram, name: &str) -> FunctionId {
    program
        .declarations
        .iter()
        .find(|declaration| declaration.name == name)
        .unwrap_or_else(|| panic!("fixture must declare `{name}`"))
        .id
}

fn storage_named(function: &MirFunctionDefinition, prefix: &str) -> StorageId {
    function
        .storage
        .iter()
        .find(|storage| storage.name.starts_with(prefix))
        .unwrap_or_else(|| panic!("fixture must contain `{prefix}` storage"))
        .id
}

fn assert_rejected(label: &str, program: &MirProgram) {
    let errors = || match verify_mir(program) {
        Ok(()) => panic!("{label} mutation must be rejected"),
        Err(errors) => errors.to_string(),
    };
    let first = errors();
    let second = errors();
    assert_eq!(first, second, "{label} diagnostics must be deterministic");
}

#[test]
fn fused_integer_matrix_contains_only_scalar_loop_machinery() {
    let program = lowered(concat!(
        "fn scan() -> unit {\n",
        "  for (byte in 1u8 .. 3u8) {}\n",
        "  for (wide in (1u .. 3u)) {}\n",
        "  for (signed in -2 .. 1) {}\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    verify_mir(&program).expect("fused integer matrix must verify");
    let function = program
        .definitions
        .get(function_id(&program, "scan"))
        .unwrap();

    assert_eq!(
        function
            .storage
            .iter()
            .filter(|storage| storage.name.starts_with("range-current"))
            .count(),
        3
    );
    assert_eq!(
        function
            .storage
            .iter()
            .filter(|storage| storage.name.starts_with("range-end"))
            .count(),
        3
    );

    let mut comparisons = Vec::new();
    let mut increments = Vec::new();
    for instruction in function
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
    {
        match instruction {
            MirInstruction::Assign(assignment) => match assignment.rvalue.kind {
                MirRvalueKind::PrimitiveComparison { operation, .. } => comparisons.push(operation),
                MirRvalueKind::Binary { operation, .. }
                    if matches!(
                        operation,
                        MirBinaryOperation::AddI64
                            | MirBinaryOperation::AddU64
                            | MirBinaryOperation::AddU8
                    ) =>
                {
                    increments.push(operation)
                }
                MirRvalueKind::OptionalPresence { .. } => {
                    panic!("fused range loop must not test an optional result")
                }
                _ => {}
            },
            MirInstruction::Call(_) | MirInstruction::OptionalSharedCleanup(_) => {
                panic!("fused range loop must not contain protocol or optional traffic")
            }
            _ => {}
        }
    }
    assert_eq!(comparisons.len(), 3);
    assert!(comparisons.iter().all(|operation| {
        operation.predicate == MirComparisonPredicate::LessThan
            && matches!(operation.operand, MirComparisonOperand::Integer(_))
    }));
    assert_eq!(increments.len(), 3);
}

#[test]
fn verifier_rejects_fused_storage_operation_and_lifetime_mutations() {
    let valid = lowered(concat!(
        "fn scan() -> unit { for (item in 1u .. 4u) {} }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    verify_mir(&valid).expect("fused range mutation seed must verify");
    let scan = function_id(&valid, "scan");

    let mut wrong_storage = valid.clone();
    let function = wrong_storage.definitions.get_mut_for_test(scan).unwrap();
    let current = storage_named(function, "range-current");
    function.storage[current.index()].ty = MirType::Bool;
    assert_rejected("wrong current type", &wrong_storage);

    let mut missing_endpoint = valid.clone();
    let function = missing_endpoint.definitions.get_mut_for_test(scan).unwrap();
    let end = storage_named(function, "range-end");
    let preheader = function.body.blocks.first_mut().unwrap();
    preheader.instructions.retain(|instruction| {
        !matches!(instruction, MirInstruction::Store(store) if store.destination == MirPlace::base(end))
    });
    assert_rejected("missing upper initialization", &missing_endpoint);

    let mut wrong_compare = valid.clone();
    let function = wrong_compare.definitions.get_mut_for_test(scan).unwrap();
    let comparison = function
        .body
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Assign(assignment) => match &mut assignment.rvalue.kind {
                MirRvalueKind::PrimitiveComparison { operation, .. } => Some(operation),
                _ => None,
            },
            _ => None,
        })
        .expect("fused header must compare current and end");
    comparison.operand = MirComparisonOperand::Integer(MirIntegerType::U8);
    assert_rejected("wrong comparison type", &wrong_compare);

    let mut wrong_increment = valid.clone();
    let function = wrong_increment.definitions.get_mut_for_test(scan).unwrap();
    let increment = function
        .body
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Assign(assignment) => match &mut assignment.rvalue.kind {
                MirRvalueKind::Binary { operation, .. }
                    if *operation == MirBinaryOperation::AddU64 =>
                {
                    Some(operation)
                }
                _ => None,
            },
            _ => None,
        })
        .expect("fused body must increment current");
    *increment = MirBinaryOperation::AddU8;
    assert_rejected("wrong increment type", &wrong_increment);

    let mut missing_cleanup = valid.clone();
    let function = missing_cleanup.definitions.get_mut_for_test(scan).unwrap();
    let current = storage_named(function, "range-current");
    let removed = function.body.blocks.iter_mut().any(|block| {
        let before = block.instructions.len();
        block.instructions.retain(|instruction| {
            !matches!(instruction, MirInstruction::StorageDead(dead) if dead.storage == current)
        });
        block.instructions.len() != before
    });
    assert!(removed, "fixture must end current storage on an exit path");
    assert_rejected("missing range cleanup", &missing_cleanup);

    let mut missing_item_epoch = valid.clone();
    let function = missing_item_epoch
        .definitions
        .get_mut_for_test(scan)
        .unwrap();
    let item = storage_named(function, "item");
    let removed = function.body.blocks.iter_mut().any(|block| {
        let before = block.instructions.len();
        block.instructions.retain(|instruction| {
            !matches!(instruction, MirInstruction::StorageLive(live) if live.storage == item)
        });
        block.instructions.len() != before
    });
    assert!(removed, "fixture must begin a fresh item epoch");
    assert_rejected("missing item epoch", &missing_item_epoch);

    let mut invalid_target = valid.clone();
    let function = invalid_target.definitions.get_mut_for_test(scan).unwrap();
    let invalid = BlockId::new(scan, function.body.blocks.len() + 1);
    let target = function
        .body
        .blocks
        .iter_mut()
        .find_map(|block| match block.terminator.as_mut() {
            Some(MirTerminator::Branch { true_target, .. }) => Some(true_target),
            _ => None,
        })
        .expect("fused range header must branch to its body");
    *target = invalid;
    assert_rejected("invalid range body target", &invalid_target);
}

#[test]
fn fused_increment_precedes_the_first_body_operation() {
    let program = lowered(concat!(
        "fn observe(value: u64) -> unit {}\n",
        "fn scan() -> unit { for (item in 1u .. 4u) { observe(item); } }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    verify_mir(&program).expect("fused range ordering fixture must verify");
    let function = program
        .definitions
        .get(function_id(&program, "scan"))
        .unwrap();
    let current = storage_named(function, "range-current");
    let body = function
        .body
        .blocks
        .iter()
        .find(|block| {
            block
                .instructions
                .iter()
                .any(|instruction| matches!(instruction, MirInstruction::Call(_)))
        })
        .expect("fixture body must contain the observable call");
    let update = body
        .instructions
        .iter()
        .position(|instruction| {
            matches!(instruction, MirInstruction::Store(store) if store.destination == MirPlace::base(current))
        })
        .expect("fused body must update its induction storage");
    let body_call = body
        .instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::Call(_)))
        .unwrap();
    assert!(
        update < body_call,
        "the range successor must execute before source body entry"
    );
}
