use crate::{
    identity::CallableId,
    mir::{
        rewrite::rewrite_program, BlockId, MirAssignment, MirBasicBlock, MirBody, MirInstruction,
        MirPathCondition, MirRvalue, MirRvalueKind, MirStorage, MirStorageKind, MirTerminator,
        MirType, MirValue, PathConditionId, StorageId, ValueId,
    },
    passes::{
        resolve_exact_mir_pass_schedule, resolve_mir_pass_schedule,
        run_mir_pipeline_with_occurrences, MirOptimizationProfile, MirPassMeasurement,
        MirPassOccurrenceOutcome,
    },
    test_support::lower_source_to_final_mir,
};

use super::*;
use crate::passes::pipeline::optimizations::{
    dead_pure_definition_elimination, whole_world_reachability,
};

fn exact_schedule(identities: &[crate::passes::MirPassIdentity]) -> crate::passes::MirPassSchedule {
    resolve_exact_mir_pass_schedule(identities).unwrap()
}

fn measurements(values: [u64; 5]) -> [MirPassMeasurement; 5] {
    [
        MirPassMeasurement::count(CONSTANT_BRANCHES, values[0]),
        MirPassMeasurement::count(SAME_TARGET_BRANCHES, values[1]),
        MirPassMeasurement::count(REMOVED_BLOCKS, values[2]),
        MirPassMeasurement::count(REMOVED_VALUES, values[3]),
        MirPassMeasurement::count(PROTECTED_UNREACHABLE_BLOCKS, values[4]),
    ]
}

#[test]
fn constant_branches_select_the_exact_target_and_remove_the_other_region() {
    for (condition, expected_result) in [(true, 11), (false, 22)] {
        let input = constant_diamond(condition);
        let entry = input.entry_function;
        let branch_span = input.definitions.get(entry).unwrap().body.blocks[0]
            .terminator
            .as_ref()
            .unwrap()
            .span();
        let measured = run_mir_pipeline_with_occurrences(input, &exact_schedule(&[IDENTITY]));
        let output = measured
            .result
            .as_ref()
            .unwrap()
            .definitions
            .get(entry)
            .unwrap();
        let record = &measured.occurrences()[0];

        assert_eq!(output.body.blocks.len(), 2);
        assert_eq!(output.values.len(), 2);
        assert!(output.storage.is_empty());
        assert!(output
            .body
            .blocks
            .iter()
            .enumerate()
            .all(|(index, block)| block.id == BlockId::new(output.callable(), index)));
        assert!(output
            .values
            .iter()
            .enumerate()
            .all(|(index, value)| value.id == ValueId::new(output.callable(), index)));
        assert!(matches!(
            output.body.blocks[0].terminator,
            Some(MirTerminator::Goto { span, .. }) if span == branch_span
        ));
        assert_eq!(returned_constant(output), expected_result);
        assert_eq!(record.outcome(), MirPassOccurrenceOutcome::Changed);
        assert_eq!(record.processed_callables(), Some(1));
        assert_eq!(record.changed_callables(), Some(1));
        assert_eq!(record.removed_mir_entities(), Some(2));
        assert_eq!(record.verification_executions(), 1);
        assert_eq!(record.measurements(), measurements([1, 0, 1, 1, 0]));
    }
}

#[test]
fn same_target_branch_folds_without_a_constant_fact() {
    let mut input = lower_source_to_final_mir(concat!(
        "fn choose(flag: bool) -> i64 { if (flag) { return 1; } else { return 2; } }\n",
        "fn main() -> i64 { return choose(true); }\n",
    ));
    let callable = input
        .definitions
        .iter()
        .find(|definition| definition.function != input.entry_function)
        .map(|definition| definition.function)
        .expect("helper function");
    let definition = input.definitions.get_mut_for_test(callable).unwrap();
    let branch = definition
        .body
        .blocks
        .iter_mut()
        .find_map(|block| match block.terminator.as_mut() {
            Some(MirTerminator::Branch {
                true_target,
                false_target,
                ..
            }) => Some((true_target, false_target)),
            _ => None,
        })
        .expect("parameter condition lowers to an ordinary branch");
    *branch.1 = *branch.0;

    let measured = run_mir_pipeline_with_occurrences(input, &exact_schedule(&[IDENTITY]));
    let output = measured
        .result
        .as_ref()
        .unwrap()
        .definitions
        .get(callable)
        .unwrap();
    let record = &measured.occurrences()[0];
    assert!(output
        .body
        .blocks
        .iter()
        .all(|block| !matches!(block.terminator, Some(MirTerminator::Branch { .. }))));
    assert_eq!(record.outcome(), MirPassOccurrenceOutcome::Changed);
    assert_eq!(
        record.measurements()[0],
        MirPassMeasurement::count(CONSTANT_BRANCHES, 0)
    );
    assert_eq!(
        record.measurements()[1],
        MirPassMeasurement::count(SAME_TARGET_BRANCHES, 1)
    );
}

#[test]
fn proof_named_branch_blocks_are_not_rewritten() {
    let mut input = constant_diamond(true);
    let entry = input.entry_function;
    let definition = input.definitions.get_mut_for_test(entry).unwrap();
    let owner = definition.callable();
    let entry_block = definition.body.entry;
    let activation = append_proof_storage(definition);
    definition.body.path_conditions.push(MirPathCondition {
        id: PathConditionId::new(owner, 0),
        parent: None,
        activation,
        active_predecessor: entry_block,
        inactive_predecessor: entry_block,
        merge: entry_block,
        span: definition.span,
    });

    let mut observed = None;
    let rewritten = rewrite_program(input, |callable, edit| {
        if callable == owner {
            observed = Some(cleanup_callable(edit)?);
        }
        Ok(())
    })
    .unwrap();
    let output = rewritten.program.definitions.get(entry).unwrap();
    assert!(matches!(
        output.body.blocks[0].terminator,
        Some(MirTerminator::Branch { .. })
    ));
    assert_eq!(output.storage.len(), 1, "CFG cleanup retains storage");
    assert_eq!(observed.unwrap(), CleanupCounts::default());
}

#[test]
fn proof_and_static_attachment_roots_retain_newly_unreachable_regions() {
    let mut proof_input = constant_diamond(true);
    let entry = proof_input.entry_function;
    let definition = proof_input.definitions.get_mut_for_test(entry).unwrap();
    let owner = definition.callable();
    let protected = BlockId::new(owner, 2);
    let activation = append_proof_storage(definition);
    definition.body.path_conditions.push(MirPathCondition {
        id: PathConditionId::new(owner, 0),
        parent: None,
        activation,
        active_predecessor: protected,
        inactive_predecessor: protected,
        merge: protected,
        span: definition.span,
    });
    let proof_counts = rewrite_one_raw(proof_input, owner);
    assert_eq!(
        proof_counts,
        CleanupCounts {
            constant_branches: 1,
            protected_unreachable_blocks: 1,
            ..CleanupCounts::default()
        }
    );

    let mut static_input = lower_source_to_final_mir(concat!(
        "class Item { static value: i64 = 1; init() {} }\n",
        "fn main() -> i64 { return Item.value; }\n",
    ));
    let initializer = static_input
        .static_lifecycle
        .as_mut()
        .and_then(|coordinator| coordinator.initializers_mut_for_test().first_mut())
        .expect("active explicit static initializer");
    let static_owner = initializer.callable();
    replace_with_constant_diamond(
        static_owner,
        &mut initializer.storage,
        &mut initializer.values,
        &mut initializer.body,
        initializer.span,
        true,
    );
    initializer.publication.initialization_exit = BlockId::new(static_owner, 1);
    initializer.publication.cleanup_entry = BlockId::new(static_owner, 2);
    let static_counts = rewrite_one_raw(static_input, static_owner);
    assert_eq!(
        static_counts,
        CleanupCounts {
            constant_branches: 1,
            protected_unreachable_blocks: 1,
            ..CleanupCounts::default()
        }
    );
}

#[test]
fn disconnected_loops_and_their_block_values_are_removed_deterministically() {
    let mut input = constant_diamond(true);
    let entry = input.entry_function;
    let definition = input.definitions.get_mut_for_test(entry).unwrap();
    let owner = definition.callable();
    let span = definition.span;
    let first = BlockId::new(owner, definition.body.blocks.len());
    let second = BlockId::new(owner, definition.body.blocks.len() + 1);
    let value = ValueId::new(owner, definition.values.len());
    definition.values.push(MirValue {
        id: value,
        ty: MirType::I64,
        span,
    });
    definition.body.blocks.push(MirBasicBlock {
        id: first,
        instructions: vec![constant_assignment(
            value,
            MirRvalueKind::ConstantI64(99),
            MirType::I64,
            span,
        )],
        terminator: Some(MirTerminator::Goto {
            target: second,
            span,
        }),
        span,
    });
    definition.body.blocks.push(MirBasicBlock {
        id: second,
        instructions: vec![],
        terminator: Some(MirTerminator::Goto {
            target: first,
            span,
        }),
        span,
    });

    let measured = run_mir_pipeline_with_occurrences(input, &exact_schedule(&[IDENTITY, IDENTITY]));
    let first_record = &measured.occurrences()[0];
    let second_record = &measured.occurrences()[1];
    assert_eq!(first_record.outcome(), MirPassOccurrenceOutcome::Changed);
    assert_eq!(first_record.measurements(), measurements([1, 0, 3, 2, 0]));
    assert_eq!(second_record.outcome(), MirPassOccurrenceOutcome::Unchanged);
    assert_eq!(second_record.verification_executions(), 0);
    assert_eq!(second_record.measurements(), measurements([0, 0, 0, 0, 0]));
}

#[test]
fn dedicated_checked_terminators_remain_byte_for_byte_unchanged() {
    let input = lower_source_to_final_mir(
        "fn divide(left: i64, right: i64) -> i64 { return left / right; } fn main() -> i64 { return divide(8, 2); }",
    );
    assert!(input.executable_definitions().any(|definition| {
        definition.body().blocks.iter().any(|block| {
            matches!(
                block.terminator,
                Some(MirTerminator::IntegerDivisorCheck { .. })
            )
        })
    }));
    let expected = input.clone();
    let processed = input.executable_definitions().count() as u64;
    let measured = run_mir_pipeline_with_occurrences(input, &exact_schedule(&[IDENTITY]));
    let record = &measured.occurrences()[0];
    assert_eq!(measured.result.as_ref().unwrap().program(), &expected);
    assert_eq!(record.outcome(), MirPassOccurrenceOutcome::Unchanged);
    assert_eq!(record.processed_callables(), Some(processed));
    assert_eq!(record.verification_executions(), 0);
    assert_eq!(record.measurements(), measurements([0, 0, 0, 0, 0]));
}

#[test]
fn cleanup_composes_with_dead_pure_and_whole_world_passes() {
    let mut input = lower_source_to_final_mir(
        "fn unused() -> i64 { return 9; } fn main() -> i64 { return 0; }",
    );
    let entry = input.entry_function;
    let definition = input.definitions.get_mut_for_test(entry).unwrap();
    replace_with_constant_diamond(
        definition.callable(),
        &mut definition.storage,
        &mut definition.values,
        &mut definition.body,
        definition.span,
        true,
    );
    let schedule = exact_schedule(&[
        IDENTITY,
        dead_pure_definition_elimination::IDENTITY,
        whole_world_reachability::IDENTITY,
    ]);

    let measured = run_mir_pipeline_with_occurrences(input, &schedule);
    let output = measured.result.as_ref().unwrap().program();
    assert_eq!(output.executable_definitions().count(), 1);
    assert!(output.definitions.get(entry).unwrap().body.blocks[0]
        .instructions
        .is_empty());
    assert_eq!(
        measured
            .occurrences()
            .iter()
            .map(|record| (record.name(), record.outcome()))
            .collect::<Vec<_>>(),
        [
            (
                "conservative-cfg-cleanup",
                MirPassOccurrenceOutcome::Changed
            ),
            (
                "dead-pure-definition-elimination",
                MirPassOccurrenceOutcome::Changed
            ),
            (
                "whole-world-reachability",
                MirPassOccurrenceOutcome::Changed
            ),
        ]
    );
}

#[test]
fn default_profile_selects_cfg_cleanup_and_later_dead_pure_cleanup() {
    let input = constant_diamond(true);
    let output = crate::passes::run_mir_pipeline(input).unwrap();
    let definition = output.definitions.get(output.entry_function).unwrap();
    assert_eq!(definition.body.blocks.len(), 2);
    assert!(definition.body.blocks[0].instructions.is_empty());
    assert!(matches!(
        definition.body.blocks[0].terminator,
        Some(MirTerminator::Goto { .. })
    ));
}

#[test]
fn unreachable_temporary_epochs_leave_valid_inert_storage_declarations() {
    let input = lower_source_to_final_mir(concat!(
        "class Value { init() {} fn touch() -> unit {} }\n",
        "fn main() -> i64 {\n",
        "  if (false) { Value().touch(); }\n",
        "  return 0;\n",
        "}\n",
    ));
    let entry = input.entry_function;
    let temporary_count = input
        .definitions
        .get(entry)
        .unwrap()
        .storage
        .iter()
        .filter(|storage| storage.kind == MirStorageKind::Temporary)
        .count();
    assert!(
        temporary_count > 0,
        "fixture must lower a produced-receiver temporary"
    );

    let measured = run_mir_pipeline_with_occurrences(input, &exact_schedule(&[IDENTITY]));
    let output = measured.result.as_ref().unwrap();
    let definition = output.definitions.get(entry).unwrap();

    assert_eq!(
        definition
            .storage
            .iter()
            .filter(|storage| storage.kind == MirStorageKind::Temporary)
            .count(),
        temporary_count,
        "CFG cleanup retains storage declarations"
    );
    assert!(definition
        .body
        .blocks
        .iter()
        .all(
            |block| block.instructions.iter().all(|instruction| !matches!(
                instruction,
                MirInstruction::StorageLive(_) | MirInstruction::StorageDead(_)
            ))
        ));
    assert_eq!(
        measured.occurrences()[0].outcome(),
        MirPassOccurrenceOutcome::Changed
    );
}

#[test]
fn default_cfg_cleanup_exposes_removed_call_targets_to_final_reachability() {
    let input = lower_source_to_final_mir(concat!(
        "fn dead_path_target() -> i64 { return 9; }\n",
        "fn main() -> i64 { if (true) { return 1; } return dead_path_target(); }\n",
    ));
    let without_cfg = run_mir_pipeline_with_occurrences(
        input.clone(),
        &resolve_mir_pass_schedule(
            MirOptimizationProfile::Default,
            ["conservative-cfg-cleanup"],
        )
        .unwrap(),
    )
    .result
    .unwrap();
    let measured = run_mir_pipeline_with_occurrences(
        input,
        &resolve_mir_pass_schedule(MirOptimizationProfile::Default, std::iter::empty()).unwrap(),
    );
    let output = measured.result.as_ref().unwrap();

    assert_eq!(without_cfg.executable_definitions().count(), 2);
    assert_eq!(output.executable_definitions().count(), 1);
    assert_eq!(
        measured
            .occurrences()
            .iter()
            .map(|record| record.name())
            .collect::<Vec<_>>(),
        [
            "dead-pure-definition-elimination",
            "primitive-constant-folding",
            "primitive-algebraic-simplification",
            "primitive-constant-folding",
            "dead-pure-definition-elimination",
            "conservative-cfg-cleanup",
            "dead-pure-definition-elimination",
            "whole-world-reachability",
        ]
    );
}

fn constant_diamond(condition: bool) -> crate::mir::MirProgram {
    let mut program = lower_source_to_final_mir("fn main() -> i64 { return 0; }");
    let entry = program.entry_function;
    let definition = program.definitions.get_mut_for_test(entry).unwrap();
    replace_with_constant_diamond(
        definition.callable(),
        &mut definition.storage,
        &mut definition.values,
        &mut definition.body,
        definition.span,
        condition,
    );
    program
}

fn replace_with_constant_diamond(
    owner: CallableId,
    storage: &mut Vec<MirStorage>,
    values: &mut Vec<MirValue>,
    body: &mut MirBody,
    span: crate::source::Span,
    condition: bool,
) {
    let block = |index| BlockId::new(owner, index);
    let value = |index| ValueId::new(owner, index);
    storage.clear();
    *values = vec![
        MirValue {
            id: value(0),
            ty: MirType::Bool,
            span,
        },
        MirValue {
            id: value(1),
            ty: MirType::I64,
            span,
        },
        MirValue {
            id: value(2),
            ty: MirType::I64,
            span,
        },
    ];
    *body = MirBody {
        entry: block(0),
        blocks: vec![
            MirBasicBlock {
                id: block(0),
                instructions: vec![constant_assignment(
                    value(0),
                    MirRvalueKind::ConstantBool(condition),
                    MirType::Bool,
                    span,
                )],
                terminator: Some(MirTerminator::Branch {
                    condition: value(0),
                    true_target: block(1),
                    false_target: block(2),
                    span,
                }),
                span,
            },
            returning_constant_block(block(1), value(1), 11, span),
            returning_constant_block(block(2), value(2), 22, span),
        ],
        path_conditions: vec![],
        logical_expressions: vec![],
    };
}

fn append_proof_storage(definition: &mut crate::mir::MirFunctionDefinition) -> StorageId {
    let identity = StorageId::new(definition.callable(), definition.storage.len());
    definition.storage.push(MirStorage {
        id: identity,
        source: None,
        name: "proof-activation".to_owned(),
        kind: MirStorageKind::PathCondition,
        ty: MirType::Bool,
        span: definition.span,
    });
    identity
}

fn returning_constant_block(
    block: BlockId,
    value: ValueId,
    constant: i64,
    span: crate::source::Span,
) -> MirBasicBlock {
    MirBasicBlock {
        id: block,
        instructions: vec![constant_assignment(
            value,
            MirRvalueKind::ConstantI64(constant),
            MirType::I64,
            span,
        )],
        terminator: Some(MirTerminator::Return {
            value: Some(value),
            span,
        }),
        span,
    }
}

fn constant_assignment(
    result: ValueId,
    kind: MirRvalueKind,
    ty: MirType,
    span: crate::source::Span,
) -> MirInstruction {
    MirInstruction::Assign(MirAssignment {
        result,
        rvalue: MirRvalue { kind, ty },
        span,
    })
}

fn returned_constant(definition: &crate::mir::MirFunctionDefinition) -> i64 {
    definition
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Assign(MirAssignment {
                rvalue:
                    MirRvalue {
                        kind: MirRvalueKind::ConstantI64(value),
                        ..
                    },
                ..
            }) if *value != 0 => Some(*value),
            _ => None,
        })
        .expect("selected return block retains its constant")
}

fn rewrite_one_raw(program: crate::mir::MirProgram, selected: CallableId) -> CleanupCounts {
    let mut observed = None;
    rewrite_program(program, |callable, edit| {
        if callable == selected {
            observed = Some(cleanup_callable(edit)?);
        }
        Ok(())
    })
    .expect("focused raw rewrite commits structurally");
    observed.expect("selected callable was rewritten")
}
