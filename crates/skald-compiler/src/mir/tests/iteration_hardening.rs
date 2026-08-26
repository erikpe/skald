//! Adversarial verification coverage for ordinary MIR produced by `for-in`.
//!
//! General iteration deliberately has no MIR identity of its own. These tests
//! mutate the ordinary calls, optionals, lifetimes, cleanup, and CFG emitted by
//! lowering and prove the shared verifier rejects every malformed relation.

use crate::{
    identity::{InterfaceId, InterfaceRequirementId},
    resolve::resolve_module_graph,
    test_support::{load_module_sources, lower_hir_to_final_mir, CANONICAL_ITER_SOURCE},
    typeck::type_check,
};

use super::*;

const ITERABLE: &str = concat!(
    "from std::iter import Iterable;\n",
    "class Counter implements Iterable<i64, u64> {\n",
    "  init() {}\n",
    "  fn iter_state() -> u64 { return 0u; }\n",
    "  fn iter_next(mut ref state: u64) -> i64? { return none; }\n",
    "}\n",
);

fn lowered(scan: &str) -> MirProgram {
    let source = format!("{ITERABLE}{scan}\nfn main() -> i64 {{ return 0; }}\n");
    let (_workspace, graph) = load_module_sources(
        "app",
        &[
            ("app.ska", source.as_str()),
            ("std/iter.ska", CANONICAL_ITER_SOURCE),
        ],
    );
    let resolved = resolve_module_graph(&graph);
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );
    let checked = type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    lower_hir_to_final_mir(&checked.hir.expect("iteration fixture must produce HIR"))
}

fn scan_id(program: &MirProgram) -> FunctionId {
    program
        .declarations
        .iter()
        .find(|declaration| declaration.name == "scan")
        .expect("fixture scan declaration must exist")
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

fn assert_rejected_deterministically(label: &str, program: &MirProgram) {
    let first = verify_mir(program)
        .expect_err("mutated iteration MIR must fail verification")
        .to_string();
    let second = verify_mir(program)
        .expect_err("mutated iteration MIR must fail repeated verification")
        .to_string();
    assert_eq!(first, second, "{label} diagnostics must be deterministic");
    assert!(!first.is_empty(), "{label} must report an actionable error");
}

#[test]
fn rejects_iteration_call_type_alias_optional_and_cfg_corruption() {
    let valid = lowered("fn scan(ref values: Counter) -> unit { for (item in values) {} }");
    verify_mir(&valid).expect("ordinary primitive iteration fixture must verify");
    let function_id = scan_id(&valid);

    let mut identity = valid.clone();
    let function = identity.definitions.get_mut_for_test(function_id).unwrap();
    let next = function
        .body
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .filter_map(|instruction| match instruction {
            MirInstruction::Call(call) if !call.arguments.is_empty() => Some(call),
            _ => None,
        })
        .next()
        .expect("iteration header must call iter_next");
    next.target = MirCallTarget::Interface(MirInterfaceCallTarget {
        interface: InterfaceId::new(usize::MAX),
        requirement: InterfaceRequirementId::new(InterfaceId::new(usize::MAX), 0),
    });
    assert_rejected_deterministically("foreign interface identity", &identity);

    let mut state_type = valid.clone();
    let function = state_type
        .definitions
        .get_mut_for_test(function_id)
        .unwrap();
    let state = storage_named(function, "iteration-state");
    function.storage[state.index()].ty = MirType::Bool;
    assert_rejected_deterministically("state type", &state_type);

    let mut result_type = valid.clone();
    let function = result_type
        .definitions
        .get_mut_for_test(function_id)
        .unwrap();
    let result = storage_named(function, "iteration-result");
    function.storage[result.index()].ty = MirType::I64;
    assert_rejected_deterministically("result type", &result_type);

    let mut alias = valid.clone();
    let function = alias.definitions.get_mut_for_test(function_id).unwrap();
    let result = storage_named(function, "iteration-result");
    let argument = function
        .body
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call) if !call.arguments.is_empty() => call.arguments.first_mut(),
            _ => None,
        })
        .expect("iter_next must receive its state alias");
    *argument = MirArgument::Place(MirPlace::base(result));
    assert_rejected_deterministically("state alias target", &alias);

    let mut optional = valid.clone();
    let function = optional.definitions.get_mut_for_test(function_id).unwrap();
    let state = storage_named(function, "iteration-state");
    let presence = function
        .body
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Assign(assignment)
                if matches!(
                    &assignment.rvalue.kind,
                    MirRvalueKind::OptionalPresence { .. }
                ) =>
            {
                Some(&mut assignment.rvalue.kind)
            }
            _ => None,
        })
        .expect("iteration header must test outer optional presence");
    let MirRvalueKind::OptionalPresence { source, .. } = presence else {
        unreachable!()
    };
    *source = MirPlace::base(state);
    assert_rejected_deterministically("outer optional layer", &optional);

    let mut nested_layer = lowered(concat!(
        "class OptionalCounter implements Iterable<i64?, u64> {\n",
        "  init() {}\n",
        "  fn iter_state() -> u64 { return 0u; }\n",
        "  fn iter_next(mut ref state: u64) -> i64?? { return none; }\n",
        "}\n",
        "fn scan(ref values: OptionalCounter) -> unit { for (item in values) {} }",
    ));
    let nested_function = scan_id(&nested_layer);
    let function = nested_layer
        .definitions
        .get_mut_for_test(nested_function)
        .unwrap();
    let result = storage_named(function, "iteration-result");
    let item = storage_named(function, "item");
    function.storage[result.index()].ty = function.storage[item.index()].ty;
    assert_rejected_deterministically("collapsed nested optional layer", &nested_layer);

    let mut destination = valid.clone();
    let function = destination
        .definitions
        .get_mut_for_test(function_id)
        .unwrap();
    let (header, cleanup) = function
        .body
        .blocks
        .iter()
        .find_map(|block| match block.terminator {
            Some(MirTerminator::Branch { false_target, .. }) => Some((block.id, false_target)),
            _ => None,
        })
        .expect("iteration header must branch to outer cleanup");
    let exit = match function.body.blocks[cleanup.index()].terminator {
        Some(MirTerminator::Goto { target, .. }) => target,
        _ => panic!("outer cleanup must reach the iteration exit"),
    };
    let Some(MirTerminator::Branch { false_target, .. }) =
        &mut function.body.blocks[header.index()].terminator
    else {
        unreachable!()
    };
    *false_target = exit;
    assert_rejected_deterministically("cleanup-skipping loop destination", &destination);
}

#[test]
fn rejects_iteration_epoch_cleanup_and_post_termination_corruption() {
    let valid = lowered("fn scan(ref values: Counter) -> unit { for (item in values) {} }");
    let function_id = scan_id(&valid);

    let mut missing_epoch = valid.clone();
    let function = missing_epoch
        .definitions
        .get_mut_for_test(function_id)
        .unwrap();
    let result = storage_named(function, "iteration-result");
    let removed = function.body.blocks.iter_mut().any(|block| {
        let before = block.instructions.len();
        block.instructions.retain(|instruction| {
            !matches!(instruction, MirInstruction::StorageLive(live) if live.storage == result)
        });
        before != block.instructions.len()
    });
    assert!(removed, "fixture must begin the repeatable result epoch");
    assert_rejected_deterministically("missing result epoch", &missing_epoch);

    let mut missing_cleanup = valid.clone();
    let function = missing_cleanup
        .definitions
        .get_mut_for_test(function_id)
        .unwrap();
    let state = storage_named(function, "iteration-state");
    let removed = function.body.blocks.iter_mut().any(|block| {
        let before = block.instructions.len();
        block.instructions.retain(|instruction| {
            !matches!(instruction, MirInstruction::StorageDead(dead) if dead.storage == state)
        });
        before != block.instructions.len()
    });
    assert!(removed, "fixture must end the retained state epoch");
    assert_rejected_deterministically("missing state cleanup", &missing_cleanup);

    let mut item_after_epoch = valid.clone();
    let function = item_after_epoch
        .definitions
        .get_mut_for_test(function_id)
        .unwrap();
    let item = storage_named(function, "item");
    let body = function
        .body
        .blocks
        .iter_mut()
        .find(|block| {
            block.instructions.iter().any(
                |instruction| matches!(instruction, MirInstruction::Store(store) if store.destination == MirPlace::base(item)),
            ) && block.instructions.iter().any(
                |instruction| matches!(instruction, MirInstruction::StorageDead(dead) if dead.storage == item),
            )
        })
        .expect("iteration body must initialize and end its item epoch");
    let dead = body
        .instructions
        .iter()
        .position(
            |instruction| matches!(instruction, MirInstruction::StorageDead(event) if event.storage == item),
        )
        .unwrap();
    let dead = body.instructions.remove(dead);
    let store = body
        .instructions
        .iter()
        .position(
            |instruction| matches!(instruction, MirInstruction::Store(operation) if operation.destination == MirPlace::base(item)),
        )
        .unwrap();
    body.instructions.insert(store, dead);
    assert_rejected_deterministically("item use outside epoch", &item_after_epoch);

    let mut call_after_termination = valid;
    let function = call_after_termination
        .definitions
        .get_mut_for_test(function_id)
        .unwrap();
    let header = function
        .body
        .blocks
        .iter()
        .find(|block| {
            block.instructions.iter().any(
                |instruction| matches!(instruction, MirInstruction::Call(call) if !call.arguments.is_empty()),
            )
        })
        .expect("iteration header must contain iter_next")
        .id;
    let cleanup = function
        .body
        .blocks
        .iter_mut()
        .find(|block| {
            block.instructions.iter().any(
                |instruction| matches!(instruction, MirInstruction::StorageDead(dead) if dead.storage == state),
            )
        })
        .expect("termination cleanup must end the retained state");
    cleanup.terminator = Some(MirTerminator::Goto {
        target: header,
        span: cleanup.span,
    });
    assert_rejected_deterministically(
        "iter_next after termination cleanup",
        &call_after_termination,
    );
}

#[test]
fn rejects_iteration_receiver_owner_anchor_and_guard_corruption() {
    let valid = lowered(concat!(
        "fn scan() -> unit {\n",
        "  for (item in Counter()) {}\n",
        "  var owner: shared Counter = new Counter();\n",
        "  for (item in *owner) {}\n",
        "  var maybe: Counter? = Counter();\n",
        "  for (item in maybe!) {}\n",
        "}\n",
    ));
    verify_mir(&valid).expect("receiver lifetime matrix must verify");
    let function_id = scan_id(&valid);

    type StoragePredicate = fn(&MirStorage) -> bool;
    let carriers: [(&str, StoragePredicate); 2] = [
        ("produced receiver owner", |storage: &MirStorage| {
            storage.name.starts_with("temporary")
        }),
        ("shared receiver anchor", |storage: &MirStorage| {
            storage.kind == MirStorageKind::SharedAnchor
        }),
    ];
    for (label, select) in carriers {
        let mut mutated = valid.clone();
        let function = mutated.definitions.get_mut_for_test(function_id).unwrap();
        let storage = function
            .storage
            .iter()
            .find(|storage| select(storage))
            .unwrap_or_else(|| panic!("fixture must contain {label}"))
            .id;
        let removed = function.body.blocks.iter_mut().any(|block| {
            let before = block.instructions.len();
            block.instructions.retain(|instruction| {
                !matches!(instruction, MirInstruction::StorageLive(live) if live.storage == storage)
            });
            before != block.instructions.len()
        });
        assert!(removed, "fixture must begin {label} lifetime");
        assert_rejected_deterministically(label, &mutated);
    }

    let mut guard = valid.clone();
    let function = guard.definitions.get_mut_for_test(function_id).unwrap();
    let removed = function.body.blocks.iter_mut().any(|block| {
        let before = block.instructions.len();
        block
            .instructions
            .retain(|instruction| !matches!(instruction, MirInstruction::EndOptionalView(_)));
        before != block.instructions.len()
    });
    assert!(removed, "fixture must end its optional receiver guard");
    assert_rejected_deterministically("missing optional receiver guard cleanup", &guard);

    let mut duplicate_cleanup = valid;
    let function = duplicate_cleanup
        .definitions
        .get_mut_for_test(function_id)
        .unwrap();
    let (block, index) = function
        .body
        .blocks
        .iter()
        .enumerate()
        .find_map(|(block, body)| {
            body.instructions
                .iter()
                .position(|instruction| matches!(instruction, MirInstruction::Cleanup(_)))
                .map(|index| (block, index))
        })
        .expect("produced receiver must have ordinary cleanup");
    let cleanup = function.body.blocks[block].instructions[index].clone();
    function.body.blocks[block]
        .instructions
        .insert(index + 1, cleanup);
    assert_rejected_deterministically("duplicate receiver cleanup", &duplicate_cleanup);
}

#[test]
fn deep_mixed_iteration_has_bounded_deterministic_verified_cfg_growth() {
    const DEPTH: usize = 24;
    let mut scan = String::from("fn scan(ref values: Counter) -> i64 { var sum: i64 = 0; ");
    for depth in 0..DEPTH {
        scan.push_str(&format!(
            "for (item{depth} in values) {{ while (false) {{ continue; }} "
        ));
    }
    scan.push_str("sum = sum + item0;");
    for _ in 0..DEPTH {
        scan.push_str(" }");
    }
    scan.push_str(" return sum; }");

    let first = lowered(&scan);
    let second = lowered(&scan);
    verify_mir(&first).expect("deep mixed iteration must verify");
    assert_eq!(dump_mir(&first), dump_mir(&second));
    let definition = first.definitions.get(scan_id(&first)).unwrap();
    assert!(
        definition.body.blocks.len() < DEPTH * 16,
        "structured loop lowering must retain linear CFG growth"
    );
}
