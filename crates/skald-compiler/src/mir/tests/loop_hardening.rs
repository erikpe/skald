use std::panic::{self, AssertUnwindSafe};

use crate::{
    backend::Target,
    hir::HirStatement,
    identity::LoopId,
    passes::run_mir_pipeline,
    test_support::{emit_assembly_without_runtime_trace as emit_assembly, run_native_assembly},
};

use super::*;

#[test]
fn condition_and_body_lifetimes_end_before_their_loop_boundaries() {
    let mir = lower_text(concat!(
        "class Token { init() {} }\n",
        "fn keep(value: i64, token: shared Token) -> bool { return value < 1; }\n",
        "fn main() -> i64 {\n",
        "  var value: i64 = 0;\n",
        "  while (keep(value, new Token())) {\n",
        "    var current: Token = Token();\n",
        "    value = value + 1;\n",
        "  }\n",
        "  return value;\n",
        "}\n",
    ));
    verify_mir(&mir).expect("loop boundary fixture must verify");
    let main = mir.definitions.get(mir.entry_function).unwrap();
    let header = main
        .body
        .blocks
        .iter()
        .find(|block| matches!(block.terminator, Some(MirTerminator::Branch { .. })))
        .expect("while header must branch");
    let Some(MirTerminator::Branch {
        true_target: body,
        false_target: exit,
        ..
    }) = header.terminator
    else {
        unreachable!();
    };
    let body = &main.body.blocks[body.index()];
    let Some(MirTerminator::Goto { target: latch, .. }) = body.terminator else {
        panic!("falling-through loop body must target its latch");
    };
    let latch = &main.body.blocks[latch.index()];

    let condition_storage: Vec<_> = main
        .storage
        .iter()
        .filter(|storage| {
            matches!(
                storage.kind,
                MirStorageKind::Argument
                    | MirStorageKind::Temporary
                    | MirStorageKind::SharedAllocation
            )
        })
        .map(|storage| storage.id)
        .collect();
    assert!(
        !condition_storage.is_empty(),
        "condition fixture must allocate compiler-owned storage"
    );
    for storage in condition_storage {
        let marker_blocks = lifetime_marker_blocks(main, storage);
        assert!(
            !marker_blocks.is_empty() && marker_blocks.iter().all(|block| *block == header.id),
            "condition storage {storage} must begin and end only in the header: {marker_blocks:?}"
        );
        assert_has_balanced_lifetime(header, storage);
    }

    let current = main
        .storage
        .iter()
        .find(|storage| storage.name == "current")
        .expect("body local must have storage")
        .id;
    assert_eq!(lifetime_marker_blocks(main, current), [body.id, body.id]);
    assert_has_balanced_lifetime(body, current);
    for boundary in [header.id, latch.id, exit] {
        assert!(
            !lifetime_marker_blocks(main, current).contains(&boundary),
            "body local must be dead at {boundary}"
        );
    }
}

#[test]
fn redirected_continue_edge_that_skips_cleanup_is_rejected_deterministically() {
    let source = concat!(
        "fn main() -> i64 {\n",
        "  var value: i64 = 0;\n",
        "  while (value < 2) {\n",
        "    var current: i64 = value;\n",
        "    if (value == 0) { value = value + 1; continue; }\n",
        "    value = value + current;\n",
        "  }\n",
        "  return value;\n",
        "}\n",
    );
    let mut mir = lower_text(source);
    let main = mir
        .definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap();
    let (continue_block, latch) = main
        .body
        .blocks
        .iter()
        .find_map(|block| match block.terminator {
            Some(MirTerminator::Goto { target, span })
                if &source[span.range().start()..span.range().end()] == "continue;" =>
            {
                Some((block.id, target))
            }
            _ => None,
        })
        .expect("fixture must contain a lowered continue edge");
    let branch = main
        .body
        .blocks
        .iter_mut()
        .find_map(|block| match block.terminator.as_mut() {
            Some(MirTerminator::Branch { true_target, .. }) if *true_target == continue_block => {
                Some(true_target)
            }
            _ => None,
        })
        .expect("conditional true edge must select continue cleanup");
    *branch = latch;

    let first = verify_mir(&mir).unwrap_err().to_string();
    let second = verify_mir(&mir).unwrap_err().to_string();
    assert_eq!(first, second);
    assert!(
        first.contains("storage lifetime state disagrees at control-flow join"),
        "{first}"
    );
}

#[test]
fn foreign_loop_identity_cannot_produce_mir() {
    let checked = type_check_source(
        "fn main() -> i64 { var value: i64 = 0; while (value < 1) { value = value + 1; } return value; }",
    );
    assert!(checked.diagnostics.is_empty());
    let mut hir = checked.hir.unwrap();
    let entry = hir.entry_function;
    let main = hir.definitions.get_mut_for_test(entry).unwrap();
    let HirStatement::While(statement) = &mut main.body.statements[1] else {
        panic!("fixture must contain typed while");
    };
    statement.loop_id = LoopId::new(FunctionId::new(99), 0);

    let result = panic::catch_unwind(AssertUnwindSafe(|| lower_hir(&hir)));
    assert!(
        result.is_err(),
        "a loop identity owned by another callable must not lower"
    );
}

#[test]
fn equivalent_split_and_renumbered_loop_cfg_survives_passes_and_backend() {
    let mut mir = lower_text(concat!(
        "fn main() -> i64 {\n",
        "  var value: i64 = 0;\n",
        "  while (value < 3) { value = value + 1; }\n",
        "  return value;\n",
        "}\n",
    ));
    insert_latch_bridge(&mut mir);
    renumber_entry_blocks(&mut mir, &[0, 4, 2, 5, 1, 3]);
    verify_mir(&mir).expect("equivalent transformed loop CFG must verify");

    let expected = mir.clone();
    assert_eq!(run_mir_pipeline(mir.clone()).unwrap(), expected);
    let assembly = emit_assembly(Target::X86_64SysV, &mir)
        .expect("backend must consume verified generic CFG without loop metadata");
    assert!(assembly.contains("jmp .Lska.fn.main.main.f0.block_"));
    assert_eq!(run_native_assembly(&assembly).code(), Some(3));
}

fn lifetime_marker_blocks(
    function: &crate::mir::MirFunctionDefinition,
    storage: StorageId,
) -> Vec<BlockId> {
    function
        .body
        .blocks
        .iter()
        .flat_map(|block| {
            block
                .instructions
                .iter()
                .filter_map(move |instruction| match instruction {
                    MirInstruction::StorageLive(event) if event.storage == storage => {
                        Some(block.id)
                    }
                    MirInstruction::StorageDead(event) if event.storage == storage => {
                        Some(block.id)
                    }
                    _ => None,
                })
        })
        .collect()
}

fn assert_has_balanced_lifetime(block: &MirBasicBlock, storage: StorageId) {
    let markers: Vec<_> = block
        .instructions
        .iter()
        .filter_map(|instruction| match instruction {
            MirInstruction::StorageLive(event) if event.storage == storage => Some(true),
            MirInstruction::StorageDead(event) if event.storage == storage => Some(false),
            _ => None,
        })
        .collect();
    assert_eq!(markers, [true, false], "{storage} must have one full epoch");
}

fn insert_latch_bridge(program: &mut MirProgram) {
    let main = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    let header = main
        .body
        .blocks
        .iter()
        .find(|block| matches!(block.terminator, Some(MirTerminator::Branch { .. })))
        .unwrap()
        .id;
    let bridge = BlockId::new(main.function, main.body.blocks.len());
    let latch = main
        .body
        .blocks
        .iter_mut()
        .find(|block| {
            matches!(
                block.terminator,
                Some(MirTerminator::Goto { target, .. }) if target == header
            ) && block.id != main.body.entry
        })
        .expect("canonical loop must have a latch");
    let span = latch.span;
    latch.terminator = Some(MirTerminator::Goto {
        target: bridge,
        span,
    });
    main.body.blocks.push(fixture_block(
        bridge,
        Vec::new(),
        Some(MirTerminator::Goto {
            target: header,
            span,
        }),
        span,
    ));
}

fn renumber_entry_blocks(program: &mut MirProgram, order: &[usize]) {
    let main = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    assert_eq!(order.len(), main.body.blocks.len());
    let mut old_to_new = vec![0; order.len()];
    for (new, old) in order.iter().copied().enumerate() {
        old_to_new[old] = new;
    }
    let remap = |block: BlockId| BlockId::new(main.function, old_to_new[block.index()]);
    let old_blocks = std::mem::take(&mut main.body.blocks);
    main.body.blocks = order
        .iter()
        .copied()
        .enumerate()
        .map(|(new, old)| {
            let mut block = old_blocks[old].clone();
            block.id = BlockId::new(main.function, new);
            match block.terminator.as_mut().unwrap() {
                MirTerminator::Goto { target, .. } => *target = remap(*target),
                MirTerminator::Branch {
                    true_target,
                    false_target,
                    ..
                } => {
                    *true_target = remap(*true_target);
                    *false_target = remap(*false_target);
                }
                MirTerminator::Return { .. } => {}
                other => panic!("loop transformation fixture has unexpected {other:?}"),
            }
            block
        })
        .collect();
    main.body.entry = remap(main.body.entry);
}
