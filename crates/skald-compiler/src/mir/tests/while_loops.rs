use crate::{
    backend::Target,
    hir::{HirStatement, HirWhile},
    identity::LoopId,
    passes::run_mir_pipeline,
    test_support::{emit_assembly_without_runtime_trace as emit_assembly, run_native_assembly},
};

use super::*;

fn lower_internal_while(source: &str, statement_index: usize) -> MirProgram {
    let checked = type_check_source(source);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let mut hir = checked.hir.unwrap();
    let entry = hir.entry_function;
    let definition = hir.definitions.get_mut_for_test(entry).unwrap();
    let statement = definition.body.statements.remove(statement_index);
    definition.body.statements.insert(
        statement_index,
        while_from_conditional(statement, LoopId::new(entry, 0)),
    );
    lower_hir(&hir)
}

fn while_from_conditional(statement: HirStatement, loop_id: LoopId) -> HirStatement {
    let HirStatement::Conditional(mut conditional) = statement else {
        panic!("internal loop fixture must place a conditional at the selected statement");
    };
    assert_eq!(
        conditional.arms.len(),
        1,
        "internal loop fixture conditional must have one arm"
    );
    assert!(
        conditional.else_block.is_none(),
        "internal loop fixture conditional must not have an else block"
    );
    let arm = conditional.arms.pop().unwrap();
    HirStatement::While(HirWhile::new(
        loop_id,
        arm.condition,
        arm.body,
        conditional.span,
    ))
}

fn counting_loop() -> MirProgram {
    lower_internal_while(
        concat!(
            "fn main() -> i64 {\n",
            "  var iterations: i64 = 0;\n",
            "  if (iterations < 3) { iterations = iterations + 1; }\n",
            "  return iterations;\n",
            "}\n",
        ),
        1,
    )
}

#[test]
fn source_while_reaches_the_canonical_mir_graph_deterministically() {
    let mir = lower_text(concat!(
        "fn main() -> i64 {\n",
        "  var iterations: i64 = 0;\n",
        "  while (iterations < 3) { iterations = iterations + 1; }\n",
        "  return iterations;\n",
        "}\n",
    ));
    verify_mir(&mir).expect("source while MIR must verify");
    assert_eq!(dump_mir(&mir), dump_mir(&mir));

    let main = mir.definitions.get(mir.entry_function).unwrap();
    let [preheader, header, body, latch, exit] = main.body.blocks.as_slice() else {
        unreachable!("source while lowering must allocate the canonical five blocks");
    };
    assert!(
        matches!(preheader.terminator, Some(MirTerminator::Goto { target, .. }) if target == header.id)
    );
    assert!(matches!(
        header.terminator,
        Some(MirTerminator::Branch {
            true_target,
            false_target,
            ..
        }) if true_target == body.id && false_target == exit.id
    ));
    assert!(
        matches!(body.terminator, Some(MirTerminator::Goto { target, .. }) if target == latch.id)
    );
    assert!(
        matches!(latch.terminator, Some(MirTerminator::Goto { target, .. }) if target == header.id)
    );
}

#[test]
fn break_targets_the_nearest_exit_and_cleans_every_exited_scope() {
    let source = concat!(
        "class Trace { init() {} destroy {} }\n",
        "fn main() -> i64 {\n",
        "  while (true) {\n",
        "    var outer: Trace = Trace();\n",
        "    while (true) { break; }\n",
        "    { var inner: Trace = Trace(); break; }\n",
        "  }\n",
        "  return 0;\n",
        "}\n",
    );
    let mir = lower_text(source);
    verify_mir(&mir).expect("break cleanup edges must verify");
    let main = mir.definitions.get(mir.entry_function).unwrap();
    let break_targets: Vec<_> = main
        .body
        .blocks
        .iter()
        .filter_map(|block| match block.terminator {
            Some(MirTerminator::Goto { target, span })
                if &source[span.range().start()..span.range().end()] == "break;" =>
            {
                Some(target)
            }
            _ => None,
        })
        .collect();
    assert_eq!(break_targets.len(), 2);
    let [inner_exit, outer_exit] = break_targets.as_slice() else {
        unreachable!();
    };
    assert_ne!(inner_exit, outer_exit);

    let outer = main
        .storage
        .iter()
        .find(|storage| storage.name == "outer")
        .unwrap()
        .id;
    let inner = main
        .storage
        .iter()
        .find(|storage| storage.name == "inner")
        .unwrap()
        .id;
    let outer_break = main
        .body
        .blocks
        .iter()
        .find(|block| {
            matches!(
                block.terminator,
                Some(MirTerminator::Goto { target, span })
                    if target == *outer_exit
                        && &source[span.range().start()..span.range().end()] == "break;"
            )
        })
        .unwrap();
    let cleanup: Vec<_> = outer_break
        .instructions
        .iter()
        .filter_map(|instruction| match instruction {
            MirInstruction::Cleanup(cleanup)
                if matches!(
                    cleanup.destination.base.expect_local_storage(),
                    storage if storage == inner || storage == outer
                ) =>
            {
                Some(cleanup.destination.base.expect_local_storage())
            }
            _ => None,
        })
        .collect();
    assert_eq!(cleanup, [inner, outer]);
    let dead: Vec<_> = outer_break
        .instructions
        .iter()
        .filter_map(|instruction| match instruction {
            MirInstruction::StorageDead(event)
                if event.storage == inner || event.storage == outer =>
            {
                Some(event.storage)
            }
            _ => None,
        })
        .collect();
    assert_eq!(dead, [inner, outer]);
}

#[test]
fn verifier_rejects_break_cleanup_that_leaves_body_storage_live_at_the_exit() {
    let mut mir = lower_text(concat!(
        "fn main() -> i64 {\n",
        "  while (true) { var local: i64 = 1; break; }\n",
        "  return 0;\n",
        "}\n",
    ));
    let entry = mir.entry_function;
    let main = mir.definitions.get_mut_for_test(entry).unwrap();
    let local = main
        .storage
        .iter()
        .find(|storage| storage.name == "local")
        .unwrap()
        .id;
    for block in &mut main.body.blocks {
        block.instructions.retain(|instruction| {
            !matches!(instruction, MirInstruction::StorageDead(event) if event.storage == local)
        });
    }

    let errors = verify_mir(&mir).unwrap_err().to_string();
    assert!(errors.contains("storage lifetime state disagrees at control-flow join"));
}

#[test]
fn continue_targets_the_latch_after_cleaning_every_exited_scope() {
    let source = concat!(
        "class Trace { init() {} destroy {} }\n",
        "fn main() -> i64 {\n",
        "  var count: i64 = 0;\n",
        "  while (count < 2) {\n",
        "    var body: Trace = Trace();\n",
        "    { var nested: Trace = Trace(); count = count + 1; continue; }\n",
        "  }\n",
        "  return count;\n",
        "}\n",
    );
    let mir = lower_text(source);
    verify_mir(&mir).expect("continue cleanup edges must verify");
    let main = mir.definitions.get(mir.entry_function).unwrap();
    let continue_block = main
        .body
        .blocks
        .iter()
        .find(|block| {
            matches!(
                block.terminator,
                Some(MirTerminator::Goto { span, .. })
                    if &source[span.range().start()..span.range().end()] == "continue;"
            )
        })
        .expect("continue must terminate its source path");
    let Some(MirTerminator::Goto { target: latch, .. }) = continue_block.terminator else {
        unreachable!();
    };
    let header = match main.body.blocks[latch.index()].terminator {
        Some(MirTerminator::Goto { target, .. }) => target,
        _ => panic!("continue target must be the loop latch"),
    };
    assert!(matches!(
        main.body.blocks[header.index()].terminator,
        Some(MirTerminator::Branch { .. })
    ));

    let body = main
        .storage
        .iter()
        .find(|storage| storage.name == "body")
        .unwrap()
        .id;
    let nested = main
        .storage
        .iter()
        .find(|storage| storage.name == "nested")
        .unwrap()
        .id;
    let cleanup: Vec<_> = continue_block
        .instructions
        .iter()
        .filter_map(|instruction| match instruction {
            MirInstruction::Cleanup(cleanup)
                if matches!(cleanup.destination.base.expect_local_storage(), storage if storage == nested || storage == body) =>
            {
                Some(cleanup.destination.base.expect_local_storage())
            }
            _ => None,
        })
        .collect();
    assert_eq!(cleanup, [nested, body]);
    let dead: Vec<_> = continue_block
        .instructions
        .iter()
        .filter_map(|instruction| match instruction {
            MirInstruction::StorageDead(event)
                if event.storage == nested || event.storage == body =>
            {
                Some(event.storage)
            }
            _ => None,
        })
        .collect();
    assert_eq!(dead, [nested, body]);
}

#[test]
fn verifier_rejects_continue_cleanup_that_leaks_body_storage_to_the_latch() {
    let source = concat!(
        "fn main() -> i64 {\n",
        "  var count: i64 = 0;\n",
        "  while (count < 1) {\n",
        "    var local: i64 = count;\n",
        "    count = count + 1;\n",
        "    continue;\n",
        "  }\n",
        "  return count;\n",
        "}\n",
    );
    let mut mir = lower_text(source);
    let entry = mir.entry_function;
    let main = mir.definitions.get_mut_for_test(entry).unwrap();
    let local = main
        .storage
        .iter()
        .find(|storage| storage.name == "local")
        .unwrap()
        .id;
    for block in &mut main.body.blocks {
        if matches!(
            block.terminator,
            Some(MirTerminator::Goto { span, .. })
                if &source[span.range().start()..span.range().end()] == "continue;"
        ) {
            block.instructions.retain(
                |instruction| !matches!(instruction, MirInstruction::StorageDead(event) if event.storage == local),
            );
        }
    }

    let errors = verify_mir(&mir).unwrap_err().to_string();
    assert!(errors.contains("storage lifetime state disagrees at control-flow join"));
}

#[test]
fn lowers_the_canonical_loop_graph_with_a_backward_generic_edge() {
    let mir = counting_loop();
    verify_mir(&mir).expect("internally constructed while MIR must verify");
    let main = mir.definitions.get(mir.entry_function).unwrap();

    assert_eq!(main.body.blocks.len(), 5);
    let [preheader, header, body, latch, exit] = main.body.blocks.as_slice() else {
        unreachable!("canonical while lowering must allocate exactly five blocks");
    };
    assert!(
        matches!(preheader.terminator, Some(MirTerminator::Goto { target, .. }) if target == header.id)
    );
    assert!(matches!(
        header.terminator,
        Some(MirTerminator::Branch {
            true_target,
            false_target,
            ..
        }) if true_target == body.id && false_target == exit.id
    ));
    assert!(
        matches!(body.terminator, Some(MirTerminator::Goto { target, .. }) if target == latch.id)
    );
    assert!(
        matches!(latch.terminator, Some(MirTerminator::Goto { target, .. }) if target == header.id)
    );
    assert!(matches!(
        exit.terminator,
        Some(MirTerminator::Return { .. })
    ));
}

#[test]
fn preserves_the_zero_iteration_exit_path() {
    let mir = lower_internal_while(
        "fn main() -> i64 { if (false) { return 7; } return 3; }\n",
        0,
    );
    verify_mir(&mir).expect("a zero-iteration internal loop must verify");
    let assembly = emit_assembly(Target::X86_64SysV, &mir).unwrap();
    assert_eq!(run_native_assembly(&assembly).code(), Some(3));
}

#[test]
fn finishes_the_condition_before_branching_and_restarts_body_local_lifetimes() {
    let mir = lower_internal_while(
        concat!(
            "class Token { init() {} }\n",
            "fn keep_going(value: i64, token: shared Token) -> bool {\n",
            "  return value < 2;\n",
            "}\n",
            "fn main() -> i64 {\n",
            "  var iterations: i64 = 0;\n",
            "  if (keep_going(iterations, new Token())) {\n",
            "    var current: i64 = iterations;\n",
            "    iterations = current + 1;\n",
            "  }\n",
            "  return iterations;\n",
            "}\n",
        ),
        1,
    );
    verify_mir(&mir).expect("body-local lifetime epochs must verify across the backedge");
    let main = mir.definitions.get(mir.entry_function).unwrap();
    let header = &main.body.blocks[1];
    let body = &main.body.blocks[2];
    let body_local = main
        .storage
        .iter()
        .find(|storage| storage.name == "current")
        .unwrap()
        .id;

    let condition_owner = header
        .instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::SharedAdopt(_)));
    let condition_end = header
        .instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::EndFullExpression(_)));
    let condition_dead = header
        .instructions
        .iter()
        .rposition(|instruction| matches!(instruction, MirInstruction::StorageDead(_)));
    assert!(
        condition_owner.is_some_and(|owner| {
            condition_end
                .is_some_and(|end| condition_dead.is_some_and(|dead| owner < end && end < dead))
        }),
        "condition-owned state must be ended before its branch"
    );
    let live = body.instructions.iter().position(
        |instruction| matches!(instruction, MirInstruction::StorageLive(event) if event.storage == body_local),
    );
    let dead = body.instructions.iter().position(
        |instruction| matches!(instruction, MirInstruction::StorageDead(event) if event.storage == body_local),
    );
    assert!(
        live.is_some_and(|live| dead.is_some_and(|dead| live < dead)),
        "the reusable body storage must have a complete lifetime epoch before the latch"
    );
}

#[test]
fn verifier_rejects_a_body_lifetime_leaking_across_the_canonical_backedge() {
    let mut mir = lower_internal_while(
        concat!(
            "fn main() -> i64 {\n",
            "  var iterations: i64 = 0;\n",
            "  if (iterations < 2) {\n",
            "    var current: i64 = iterations;\n",
            "    iterations = current + 1;\n",
            "  }\n",
            "  return iterations;\n",
            "}\n",
        ),
        1,
    );
    let entry = mir.entry_function;
    let main = mir.definitions.get_mut_for_test(entry).unwrap();
    let current = main
        .storage
        .iter()
        .find(|storage| storage.name == "current")
        .unwrap()
        .id;
    for block in &mut main.body.blocks {
        block.instructions.retain(|instruction| {
            !matches!(instruction, MirInstruction::StorageDead(event) if event.storage == current)
        });
    }

    let errors = verify_mir(&mir).unwrap_err().to_string();
    assert!(errors.contains("storage lifetime state disagrees at control-flow join"));
}

#[test]
fn loop_dump_pipeline_backend_and_native_execution_accept_the_backedge() {
    let mir = counting_loop();
    let expected = mir.clone();
    assert_eq!(
        run_mir_pipeline(mir.clone()).unwrap().program(),
        &expected,
        "the target-independent pass boundary must preserve verified loop CFG"
    );

    let dump = dump_mir(&mir);
    assert_eq!(dump, dump_mir(&mir));
    let control_flow: Vec<_> = dump
        .lines()
        .filter(|line| {
            line.trim_start().starts_with("f0:b")
                || line.trim_start().starts_with("goto f0:")
                || line.trim_start().starts_with("branch f0:")
                || line.trim_start().starts_with("return f0:")
        })
        .map(|line| line.split(" @").next().unwrap().trim())
        .collect();
    assert_eq!(
        control_flow,
        [
            "f0:b0",
            "goto f0:b1",
            "f0:b1",
            "branch f0:v3, true f0:b2, false f0:b4",
            "f0:b2",
            "goto f0:b3",
            "f0:b3",
            "goto f0:b1",
            "f0:b4",
            "return f0:v7",
        ]
    );

    let assembly = emit_assembly(Target::X86_64SysV, &mir)
        .expect("the generic loop CFG must reach the native backend");
    assert!(
        assembly.contains("jmp .Lska.fn.main.main.f0.block_1"),
        "assembly must retain a mechanical backward jump to the header"
    );
    assert_eq!(run_native_assembly(&assembly).code(), Some(3));
}

#[test]
fn lowers_nested_loops_and_omits_an_unreachable_latch_after_a_returning_body() {
    let checked = type_check_source(concat!(
        "fn main() -> i64 {\n",
        "  var outer: i64 = 0;\n",
        "  var inner: i64 = 0;\n",
        "  if (outer < 2) {\n",
        "    if (inner < 2) { inner = inner + 1; }\n",
        "    outer = outer + 1;\n",
        "  }\n",
        "  return outer + inner;\n",
        "}\n",
    ));
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let mut hir = checked.hir.unwrap();
    let entry = hir.entry_function;
    let definition = hir.definitions.get_mut_for_test(entry).unwrap();
    let HirStatement::Conditional(mut outer) = definition.body.statements.remove(2) else {
        unreachable!("fixture outer conditional must type-check as HIR conditional");
    };
    let mut outer_arm = outer.arms.pop().unwrap();
    let inner = outer_arm.body.statements.remove(0);
    outer_arm
        .body
        .statements
        .insert(0, while_from_conditional(inner, LoopId::new(entry, 1)));
    definition.body.statements.insert(
        2,
        HirStatement::While(HirWhile::new(
            LoopId::new(entry, 0),
            outer_arm.condition,
            outer_arm.body,
            outer.span,
        )),
    );

    let nested = lower_hir(&hir);
    verify_mir(&nested).expect("nested internal loops must verify");
    let assembly = emit_assembly(Target::X86_64SysV, &nested)
        .expect("nested generic loop CFG must reach the backend");
    assert_eq!(run_native_assembly(&assembly).code(), Some(4));

    let returning = lower_internal_while(
        "fn main() -> i64 { if (true) { return 7; } return 3; }\n",
        0,
    );
    verify_mir(&returning).expect("a returning loop body must not create an invalid latch edge");
    let main = returning.definitions.get(returning.entry_function).unwrap();
    assert_eq!(main.body.blocks.len(), 4);
    assert!(matches!(
        main.body.blocks[2].terminator,
        Some(MirTerminator::Return { .. })
    ));
    assert!(matches!(
        main.body.blocks[3].terminator,
        Some(MirTerminator::Return { .. })
    ));
    let assembly = emit_assembly(Target::X86_64SysV, &returning).unwrap();
    assert_eq!(run_native_assembly(&assembly).code(), Some(7));
}

#[test]
fn repeats_every_current_ownership_and_compiler_storage_family_safely() {
    let mir = lower_internal_while(
        concat!(
            "class Item { init() {} fn read() -> i64 { return 1; } }\n",
            "class Holder {\n",
            "  edge: shared Obj;\n",
            "  init() { self.edge = new Item(); }\n",
            "}\n",
            "fn main() -> i64 {\n",
            "  var iterations: i64 = 0;\n",
            "  var holder: Holder = Holder();\n",
            "  if (iterations < 2) {\n",
            "    var inline: Item = Item();\n",
            "    var owner: shared Item = new Item();\n",
            "    var primitive_optional: i64? = iterations;\n",
            "    var class_optional: Item? = Item();\n",
            "    var optional_owner: shared? Item = new Item();\n",
            "    var values: i64[] = i64[](1u);\n",
            "    values[0] = primitive_optional!;\n",
            "    iterations = iterations + ((Item) *holder.edge).read();\n",
            "  }\n",
            "  return iterations;\n",
            "}\n",
        ),
        2,
    );
    verify_mir(&mir).expect("all current storage families must restart across a loop backedge");
    let main = mir.definitions.get(mir.entry_function).unwrap();

    for name in [
        "inline",
        "owner",
        "primitive_optional",
        "class_optional",
        "optional_owner",
        "values",
    ] {
        let storage = main
            .storage
            .iter()
            .find(|storage| storage.name == name)
            .unwrap()
            .id;
        let live = main.body.blocks.iter().any(|block| {
            block.instructions.iter().any(
                |instruction| matches!(instruction, MirInstruction::StorageLive(event) if event.storage == storage),
            )
        });
        let dead = main.body.blocks.iter().any(|block| {
            block.instructions.iter().any(
                |instruction| matches!(instruction, MirInstruction::StorageDead(event) if event.storage == storage),
            )
        });
        assert!(live && dead, "{name} must have a complete reusable epoch");
    }

    for expected in [
        MirStorageKind::SharedAnchor,
        MirStorageKind::OptionalUnwrap,
        MirStorageKind::ArrayBacking,
        MirStorageKind::ArrayProduced,
    ] {
        assert!(
            main.storage.iter().any(|storage| storage.kind == expected),
            "loop fixture must exercise {expected:?}"
        );
    }
    assert!(
        main.storage
            .iter()
            .any(|storage| matches!(storage.kind, MirStorageKind::CheckedView(_))),
        "loop fixture must exercise checked-view storage"
    );
    run_mir_pipeline(mir.clone()).expect("ownership-heavy loop must survive MIR passes");
    emit_assembly(Target::X86_64SysV, &mir)
        .expect("ownership-heavy generic loop CFG must reach the backend");
}
