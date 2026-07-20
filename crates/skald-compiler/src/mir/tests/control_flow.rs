use super::*;

#[test]
fn lowers_exhaustive_conditionals_without_an_unreachable_join() {
    let mir = lower_text(concat!(
        "fn choose(first: bool, second: bool) -> i64 {\n",
        "  if (first) { return 1; }\n",
        "  elif (second) { return 2; }\n",
        "  else { return 3; }\n",
        "}\n",
        "fn main() -> i64 { return choose(false, true); }\n",
    ));

    assert!(verify_mir(&mir).is_ok());
    let choose = mir.definitions.get(FunctionId::new(0)).unwrap();
    assert_eq!(choose.body.blocks.len(), 5);
    assert!(matches!(
        choose.body.blocks[0].terminator,
        Some(MirTerminator::Branch {
            true_target,
            false_target,
            ..
        }) if true_target == choose.body.blocks[1].id
            && false_target == choose.body.blocks[2].id
    ));
    assert!(matches!(
        choose.body.blocks[2].terminator,
        Some(MirTerminator::Branch {
            true_target,
            false_target,
            ..
        }) if true_target == choose.body.blocks[3].id
            && false_target == choose.body.blocks[4].id
    ));
    for index in [1, 3, 4] {
        assert!(matches!(
            choose.body.blocks[index].terminator,
            Some(MirTerminator::Return { .. })
        ));
    }

    let dump = dump_mir(&mir);
    let control_flow: Vec<_> = dump
        .lines()
        .filter(|line| {
            line.contains("EntryBlock f0:b")
                || line.trim_start().starts_with("f0:b")
                || line.trim_start().starts_with("branch f0:")
                || line.trim_start().starts_with("return f0:")
        })
        .map(|line| line.split(" @").next().unwrap().trim())
        .collect();
    assert_eq!(
        control_flow,
        [
            "EntryBlock f0:b0",
            "f0:b0",
            "branch f0:v0, true f0:b1, false f0:b2",
            "f0:b1",
            "return f0:v1",
            "f0:b2",
            "branch f0:v2, true f0:b3, false f0:b4",
            "f0:b3",
            "return f0:v3",
            "f0:b4",
            "return f0:v4",
        ]
    );
}

#[test]
fn lowers_fallthrough_arms_through_storage_to_one_join() {
    let mir = lower_text(concat!(
        "fn main() -> i64 {\n",
        "  var result: i64 = 7;\n",
        "  if (true) {} else {}\n",
        "  return result;\n",
        "}\n",
    ));

    assert!(verify_mir(&mir).is_ok());
    let main = mir.definitions.get(mir.entry_function).unwrap();
    assert_eq!(main.body.blocks.len(), 4);
    let join = main.body.blocks[3].id;
    assert!(main.body.blocks[1..=2].iter().all(|block| {
        matches!(block.terminator, Some(MirTerminator::Goto { target, .. }) if target == join)
    }));
    assert!(matches!(
        main.body.blocks[3].terminator,
        Some(MirTerminator::Return { .. })
    ));
}

#[test]
fn lowers_condition_calls_in_source_order_on_the_false_continuation_chain() {
    let mir = lower_text(concat!(
        "fn first() -> bool { return false; }\n",
        "fn second() -> bool { return true; }\n",
        "fn main() -> i64 {\n",
        "  if (first()) { return 1; }\n",
        "  elif (second()) { return 2; }\n",
        "  else { return 3; }\n",
        "}\n",
    ));

    assert!(verify_mir(&mir).is_ok());
    let main = mir.definitions.get(mir.entry_function).unwrap();
    let targets: Vec<_> = main
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match instruction {
            MirInstruction::Call(MirCall {
                target: MirCallTarget::Direct(target),
                ..
            }) => Some(*target),
            _ => None,
        })
        .collect();
    assert_eq!(targets, [FunctionId::new(0), FunctionId::new(1)]);
    assert!(matches!(
        main.body.blocks[0].terminator,
        Some(MirTerminator::Branch { false_target, .. })
            if false_target == main.body.blocks[2].id
    ));
}

#[test]
fn lowering_discards_statements_after_an_unconditional_return() {
    let mir = lower_text("fn main() -> i64 { { return 1; } return 2; }");
    let main = mir.definitions.get(mir.entry_function).unwrap();
    let block = main.block(main.body.entry).unwrap();

    assert_eq!(main.values.len(), 1);
    assert_eq!(block.instructions.len(), 1);
    assert!(matches!(
        block.terminator,
        Some(MirTerminator::Return { .. })
    ));
}
