use crate::{
    identity::CallableId,
    mir::{
        BlockId, MirAssignment, MirBasicBlock, MirBody, MirInstruction, MirRvalue, MirRvalueKind,
        MirTerminator, MirType, MirValue, ValueId,
    },
    passes::{
        reachability::analyze_reachability, resolve_exact_mir_pass_schedule,
        resolve_mir_pass_schedule, run_mir_pipeline_with_occurrences, MirOptimizationProfile,
        MirPassMeasurement, MirPassOccurrenceOutcome, MirPassStage,
    },
    test_support::lower_source_to_final_mir,
};

use super::*;
use crate::passes::pipeline::optimizations::{
    conservative_cfg_cleanup, primitive_constant_folding, whole_world_reachability,
};

fn exact_schedule(identities: &[crate::passes::MirPassIdentity]) -> crate::passes::MirPassSchedule {
    resolve_exact_mir_pass_schedule(identities).unwrap()
}

#[test]
fn disconnected_region_is_removed_and_dense_identities_are_rebuilt() {
    let input = disconnected_program();
    let entry = input.entry_function;
    let measured = run_mir_pipeline_with_occurrences(input, &exact_schedule(&[IDENTITY]));
    let verified = measured.result.as_ref().unwrap();
    let output = verified.definitions.get(entry).unwrap();
    let record = &measured.occurrences()[0];

    assert_eq!(output.body.blocks.len(), 1);
    assert_eq!(output.values.len(), 1);
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
    assert_eq!(record.stage(), MirPassStage::Final);
    assert_eq!(record.outcome(), MirPassOccurrenceOutcome::Changed);
    assert_eq!(record.processed_callables(), Some(1));
    assert_eq!(record.changed_callables(), Some(1));
    assert_eq!(record.removed_mir_entities(), Some(2));
    assert_eq!(
        verified.reachability(),
        &analyze_reachability(verified.program()).unwrap()
    );
    assert_eq!(
        record.measurements(),
        [
            MirPassMeasurement::count(REMOVED_BLOCKS, 1),
            MirPassMeasurement::count(REMOVED_VALUES, 1),
            MirPassMeasurement::count(RETAINED_PERMANENT_ROOTS, 0),
        ]
    );
}

#[test]
fn repeated_occurrence_changes_once_then_reports_exact_unchanged_counts() {
    let measured = run_mir_pipeline_with_occurrences(
        disconnected_program(),
        &exact_schedule(&[IDENTITY, IDENTITY]),
    );

    assert!(measured.result.is_ok());
    assert_eq!(measured.statistics.verification_executions(), 3);
    assert_eq!(measured.statistics.processed_callables(), 2);
    assert_eq!(measured.statistics.changed_callables(), 1);
    assert_eq!(
        measured
            .occurrences()
            .iter()
            .map(|record| (record.occurrence(), record.outcome()))
            .collect::<Vec<_>>(),
        [
            (0, MirPassOccurrenceOutcome::Changed),
            (1, MirPassOccurrenceOutcome::Unchanged),
        ]
    );
    assert_eq!(
        measured.occurrences()[1].measurements(),
        [
            MirPassMeasurement::count(REMOVED_BLOCKS, 0),
            MirPassMeasurement::count(REMOVED_VALUES, 0),
            MirPassMeasurement::count(RETAINED_PERMANENT_ROOTS, 0),
        ]
    );
}

#[test]
fn consumed_logical_roots_release_dead_cfg_after_proof_rich_cleanup() {
    let source = logical_dead_source();
    let proof_cleanup = [
        primitive_constant_folding::IDENTITY,
        conservative_cfg_cleanup::IDENTITY,
    ];
    let without_canary = run_mir_pipeline_with_occurrences(
        lower_source_to_final_mir(source),
        &exact_schedule(&proof_cleanup),
    )
    .result
    .unwrap();
    let with_canary = run_mir_pipeline_with_occurrences(
        lower_source_to_final_mir(source),
        &exact_schedule(&[
            primitive_constant_folding::IDENTITY,
            conservative_cfg_cleanup::IDENTITY,
            IDENTITY,
        ]),
    );
    let output = with_canary.result.as_ref().unwrap();
    let record = with_canary.occurrences().last().unwrap();

    assert!(block_count(output) < block_count(&without_canary));
    assert_eq!(record.outcome(), MirPassOccurrenceOutcome::Changed);
    assert!(measurement(record, REMOVED_BLOCKS) > 0);
}

#[test]
fn final_reachability_observes_call_sites_removed_with_the_dead_region() {
    let input = lower_source_to_final_mir(logical_dead_source());
    let selected = input
        .declarations
        .iter()
        .find(|declaration| declaration.name == "selected")
        .unwrap()
        .id;
    let without_canary = run_mir_pipeline_with_occurrences(
        input.clone(),
        &exact_schedule(&[
            primitive_constant_folding::IDENTITY,
            conservative_cfg_cleanup::IDENTITY,
            whole_world_reachability::IDENTITY,
        ]),
    )
    .result
    .unwrap();
    let with_canary = run_mir_pipeline_with_occurrences(
        input,
        &exact_schedule(&[
            primitive_constant_folding::IDENTITY,
            conservative_cfg_cleanup::IDENTITY,
            IDENTITY,
            whole_world_reachability::IDENTITY,
        ]),
    )
    .result
    .unwrap();

    assert!(without_canary.definitions.get(selected).is_some());
    assert!(with_canary.definitions.get(selected).is_none());
}

#[test]
fn reachable_empty_branches_loops_and_checked_failures_remain_unchanged() {
    for source in [
        "fn main() -> i64 { if (true) {} return 0; }",
        "fn main() -> i64 { while (false) {} return 0; }",
        "fn divide(left: i64, right: i64) -> i64 { return left / right; } fn main() -> i64 { return divide(8, 2); }",
    ] {
        let measured = run_mir_pipeline_with_occurrences(
            lower_source_to_final_mir(source),
            &exact_schedule(&[IDENTITY]),
        );
        let record = &measured.occurrences()[0];
        assert!(measured.result.is_ok(), "{source}");
        assert_eq!(record.outcome(), MirPassOccurrenceOutcome::Unchanged, "{source}");
        assert_eq!(measurement(record, REMOVED_BLOCKS), 0, "{source}");
    }
}

#[test]
fn functions_members_and_static_initializers_share_the_same_final_cfg_rewrite() {
    let mut input = lower_source_to_final_mir(concat!(
        "class Item {\n",
        "  static seed: i64 = 1;\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  copy(ref other: Item) { self.value = other.value; }\n",
        "  assign(ref other: Item) { self.value = other.value; }\n",
        "  destroy {}\n",
        "  fn read() -> i64 { return self.value; }\n",
        "  static fn identity(value: i64) -> i64 { return value; }\n",
        "}\n",
        "fn main() -> i64 { var item: Item = Item(2); return Item.identity(item.read()) + Item.seed; }\n",
    ));
    let expected_lifecycle = input.static_lifecycle.clone();
    let function_ids = input
        .definitions
        .iter()
        .map(|definition| definition.function)
        .collect::<Vec<_>>();
    for function in function_ids {
        append_disconnected_loop(&mut input.definitions.get_mut_for_test(function).unwrap().body);
    }
    let member_ids = input
        .member_definitions
        .iter()
        .map(|definition| definition.callable)
        .collect::<Vec<_>>();
    for callable in member_ids {
        append_disconnected_loop(
            &mut input
                .member_definitions
                .get_mut_for_test(callable)
                .unwrap()
                .body,
        );
    }
    if let Some(coordinator) = &mut input.static_lifecycle {
        for initializer in coordinator.initializers_mut_for_test() {
            append_disconnected_loop(&mut initializer.body);
        }
    }
    let callable_count = input.executable_definitions().count();

    let measured = run_mir_pipeline_with_occurrences(input, &exact_schedule(&[IDENTITY]));
    let output = measured.result.as_ref().unwrap();
    let record = &measured.occurrences()[0];

    assert_eq!(record.processed_callables(), Some(callable_count as u64));
    assert_eq!(record.changed_callables(), Some(callable_count as u64));
    assert_eq!(measurement(record, REMOVED_BLOCKS), callable_count as u64);
    assert_eq!(output.static_lifecycle, expected_lifecycle);
}

#[test]
fn registered_canary_is_selectable_but_not_in_the_default_profile() {
    let default =
        resolve_mir_pass_schedule(MirOptimizationProfile::Default, std::iter::empty()).unwrap();
    assert!(default
        .iter()
        .all(|occurrence| occurrence.identity() != IDENTITY));

    let excluded = resolve_mir_pass_schedule(MirOptimizationProfile::Default, [NAME]).unwrap();
    assert_eq!(excluded, default);

    let exact = exact_schedule(&[IDENTITY]);
    assert_eq!(exact.normalization_position(), 0);
    assert_eq!(exact.as_slice()[0].name(), NAME);
    assert_eq!(exact.as_slice()[0].stage(), MirPassStage::Final);
}

fn measurement(record: &crate::passes::MirPassOccurrenceRecord, name: &str) -> u64 {
    record
        .measurements()
        .iter()
        .find(|measurement| measurement.name() == name)
        .unwrap()
        .value()
}

fn block_count(program: &crate::passes::VerifiedFinalMirProgram) -> usize {
    program
        .executable_definitions()
        .map(|definition| definition.body().blocks.len())
        .sum()
}

fn disconnected_program() -> crate::mir::MirProgram {
    let mut program = lower_source_to_final_mir("fn main() -> i64 { return 0; }");
    let definition = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    let owner = CallableId::Function(definition.function);
    let span = definition.span;
    let value = ValueId::new(owner, definition.values.len());
    definition.values.push(MirValue {
        id: value,
        ty: MirType::I64,
        span,
    });
    definition.body.blocks.push(MirBasicBlock {
        id: BlockId::new(owner, definition.body.blocks.len()),
        instructions: vec![MirInstruction::Assign(MirAssignment {
            result: value,
            rvalue: MirRvalue {
                kind: MirRvalueKind::ConstantI64(99),
                ty: MirType::I64,
            },
            span,
        })],
        terminator: Some(MirTerminator::Return {
            value: Some(value),
            span,
        }),
        span,
    });
    program
}

fn append_disconnected_loop(body: &mut MirBody) {
    let owner = body.entry.callable();
    let block = BlockId::new(owner, body.blocks.len());
    let span = body.blocks[0].span;
    body.blocks.push(MirBasicBlock {
        id: block,
        instructions: vec![],
        terminator: Some(MirTerminator::Goto {
            target: block,
            span,
        }),
        span,
    });
}

fn logical_dead_source() -> &'static str {
    concat!(
        "fn selected() -> bool { return true; }\n",
        "fn main() -> i64 {\n",
        "  if (true) { return 1; }\n",
        "  if (false && selected()) { return 2; }\n",
        "  return 3;\n",
        "}\n",
    )
}
