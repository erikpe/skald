use crate::{
    identity::CallableId,
    mir::{
        BlockId, MirAssignment, MirBasicBlock, MirBody, MirFunctionDefinition, MirInstruction,
        MirRvalue, MirRvalueKind, MirTerminator, MirType, MirValue, ValueId,
    },
    passes::{
        reachability::analyze_reachability, resolve_exact_mir_pass_schedule,
        resolve_mir_pass_schedule, run_mir_pipeline_with_occurrences, verify_final_mir,
        MirOptimizationProfile, MirPassMeasurement, MirPassOccurrenceOutcome, MirPassStage,
    },
    test_support::lower_source_to_final_mir,
};

use super::*;
use crate::passes::pipeline::optimizations::{
    post_proof_unreachable_block_elimination, whole_world_reachability,
};
use crate::passes::pipeline::run_mir_pipeline_measured_inspected;

#[test]
fn forwards_complete_transitive_plan_and_reports_exact_changes() {
    let input = branching_chain_program();
    let entry = input.entry_function;
    let measured = run_mir_pipeline_with_occurrences(input, &schedule(&[IDENTITY]));
    let output = measured.result.as_ref().unwrap();
    let definition = output.definitions.get(entry).unwrap();
    let record = &measured.occurrences()[0];

    assert_eq!(definition.body.blocks.len(), 2);
    assert!(matches!(
        definition.body.blocks[0].terminator,
        Some(MirTerminator::Branch {
            true_target,
            false_target,
            ..
        }) if true_target == block(definition.callable(), 1)
            && false_target == block(definition.callable(), 1)
    ));
    assert_eq!(record.name(), NAME);
    assert_eq!(record.stage(), MirPassStage::Final);
    assert_eq!(record.outcome(), MirPassOccurrenceOutcome::Changed);
    assert_eq!(record.processed_callables(), Some(1));
    assert_eq!(record.changed_callables(), Some(1));
    assert_eq!(record.removed_mir_entities(), Some(2));
    assert_eq!(
        record.measurements(),
        [
            MirPassMeasurement::count(REMOVED_FORWARDING_BLOCKS, 2),
            MirPassMeasurement::count(REDIRECTED_SUCCESSOR_OCCURRENCES, 3),
            MirPassMeasurement::count(RETAINED_CYCLIC_BLOCKS, 0),
            MirPassMeasurement::count(RETAINED_PERMANENT_ATTACHMENT_BARRIERS, 0),
        ]
    );
    assert_eq!(
        output.reachability(),
        &analyze_reachability(output.program()).unwrap()
    );
}

#[test]
fn repeated_occurrence_is_idempotent_and_checkpoints_every_verified_product() {
    let input = branching_chain_program();
    let exact = schedule(&[IDENTITY, IDENTITY]);
    let mut labels = Vec::new();
    let mut dumps = Vec::new();
    let mut inspector = |checkpoint: crate::passes::MirPipelineCheckpoint<'_>| {
        labels.push(checkpoint.label().to_string());
        if let crate::passes::MirPipelineCheckpoint::Final(checkpoint) = checkpoint {
            dumps.push(crate::mir::dump_mir(checkpoint.verified()));
        }
    };
    let inspected =
        run_mir_pipeline_measured_inspected(input.clone(), &exact, Some(&mut inspector));
    assert!(inspected.result.is_ok());
    assert_eq!(
        labels,
        [
            "proof-rich-input",
            "after-proof-normalization",
            "after-final-0-post-proof-empty-block-forwarding-0",
            "after-final-1-post-proof-empty-block-forwarding-1",
            "final",
        ]
    );
    assert_ne!(dumps[0], dumps[1]);
    assert_eq!(dumps[1], dumps[2]);
    assert_eq!(dumps[2], dumps[3]);

    let measured = run_mir_pipeline_with_occurrences(input, &exact);
    assert_eq!(measured.statistics.verification_executions(), 3);
    assert_eq!(
        measured
            .occurrences()
            .iter()
            .map(|record| record.outcome())
            .collect::<Vec<_>>(),
        [
            MirPassOccurrenceOutcome::Changed,
            MirPassOccurrenceOutcome::Unchanged,
        ]
    );
    assert_eq!(
        measurement(&measured.occurrences()[1], REMOVED_FORWARDING_BLOCKS),
        0
    );
}

#[test]
fn cycles_and_chains_entering_cycles_are_retained_without_resealing() {
    let input = cyclic_program();
    let expected = verify_final_mir(input.clone()).unwrap();
    let measured = run_mir_pipeline_with_occurrences(input, &schedule(&[IDENTITY]));
    let record = &measured.occurrences()[0];

    assert_eq!(measured.result.as_ref().unwrap(), &expected);
    assert_eq!(measured.statistics.verification_executions(), 2);
    assert_eq!(record.outcome(), MirPassOccurrenceOutcome::Unchanged);
    assert_eq!(measurement(record, REMOVED_FORWARDING_BLOCKS), 0);
    assert_eq!(measurement(record, REDIRECTED_SUCCESSOR_OCCURRENCES), 0);
    assert_eq!(measurement(record, RETAINED_CYCLIC_BLOCKS), 4);
}

#[test]
fn functions_members_and_static_initializers_use_one_atomic_pass() {
    let mut input = lower_source_to_final_mir(concat!(
        "class Item {\n",
        "  static seed: i64 = 1;\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  fn read() -> i64 { return self.value; }\n",
        "}\n",
        "fn helper(value: i64) -> i64 { return value; }\n",
        "fn main() -> i64 { var item: Item = Item(2); return helper(item.read() + Item.seed); }\n",
    ));
    let functions = input
        .definitions
        .iter()
        .map(|definition| definition.function)
        .collect::<Vec<_>>();
    for function in functions {
        append_unreachable_forwarding_block(
            &mut input.definitions.get_mut_for_test(function).unwrap().body,
        );
    }
    let members = input
        .member_definitions
        .iter()
        .map(|definition| definition.callable)
        .collect::<Vec<_>>();
    for callable in members {
        append_unreachable_forwarding_block(
            &mut input
                .member_definitions
                .get_mut_for_test(callable)
                .unwrap()
                .body,
        );
    }
    for initializer in input
        .static_lifecycle
        .as_mut()
        .unwrap()
        .initializers_mut_for_test()
    {
        append_unreachable_forwarding_block(&mut initializer.body);
    }
    let callable_count = input.executable_definitions().count();

    let measured = run_mir_pipeline_with_occurrences(input, &schedule(&[IDENTITY]));
    let record = &measured.occurrences()[0];
    assert!(measured.result.is_ok());
    assert_eq!(record.processed_callables(), Some(callable_count as u64));
    assert_eq!(record.changed_callables(), Some(callable_count as u64));
    assert_eq!(
        measurement(record, REMOVED_FORWARDING_BLOCKS),
        callable_count as u64
    );
    assert!(
        measurement(record, RETAINED_PERMANENT_ATTACHMENT_BARRIERS) > 0,
        "measurements: {:?}",
        record.measurements()
    );
}

#[test]
fn forwarding_operates_on_entry_unreachable_regions_without_the_unreachable_canary() {
    let mut input = lower_source_to_final_mir("fn main() -> i64 { return 7; }");
    let entry = input.entry_function;
    append_unreachable_forwarding_block(
        &mut input.definitions.get_mut_for_test(entry).unwrap().body,
    );

    let measured = run_mir_pipeline_with_occurrences(input, &schedule(&[IDENTITY]));
    let output = measured
        .result
        .as_ref()
        .unwrap()
        .definitions
        .get(entry)
        .unwrap();
    assert_eq!(
        measurement(&measured.occurrences()[0], REMOVED_FORWARDING_BLOCKS),
        1
    );
    assert_eq!(output.body.blocks.len(), 1);
}

#[test]
fn default_registration_is_selectable_between_unreachable_cleanup_and_reachability() {
    let default =
        resolve_mir_pass_schedule(MirOptimizationProfile::Default, std::iter::empty()).unwrap();
    let occurrence = default
        .iter()
        .find(|occurrence| occurrence.identity() == IDENTITY)
        .unwrap();
    assert_eq!(occurrence.position(), 9);
    assert_eq!(occurrence.stage(), MirPassStage::Final);
    assert_eq!(
        default.as_slice()[8].identity(),
        post_proof_unreachable_block_elimination::IDENTITY
    );
    assert_eq!(
        default.as_slice()[10].identity(),
        whole_world_reachability::IDENTITY
    );

    let excluded = resolve_mir_pass_schedule(MirOptimizationProfile::Default, [NAME]).unwrap();
    assert_eq!(excluded.len(), default.len() - 1);
    assert!(excluded
        .iter()
        .all(|occurrence| occurrence.identity() != IDENTITY));

    let exact = schedule(&[IDENTITY]);
    assert_eq!(exact.normalization_position(), 0);
    assert_eq!(exact.as_slice()[0].name(), NAME);
    assert_eq!(exact.as_slice()[0].stage(), MirPassStage::Final);
}

fn schedule(identities: &[crate::passes::MirPassIdentity]) -> crate::passes::MirPassSchedule {
    resolve_exact_mir_pass_schedule(identities).unwrap()
}

fn measurement(record: &crate::passes::MirPassOccurrenceRecord, name: &str) -> u64 {
    record
        .measurements()
        .iter()
        .find(|measurement| measurement.name() == name)
        .unwrap_or_else(|| panic!("missing `{name}` pass measurement"))
        .value()
}

fn branching_chain_program() -> crate::mir::MirProgram {
    let mut program = lower_source_to_final_mir("fn main() -> i64 { return 7; }");
    let definition = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    let owner = definition.callable();
    let span = definition.span;
    let mut result = take_single_entry(definition);
    result.id = block(owner, 3);
    let condition = ValueId::new(owner, definition.values.len());
    definition.values.push(MirValue {
        id: condition,
        ty: MirType::Bool,
        span,
    });
    definition.body.entry = block(owner, 0);
    definition.body.blocks = vec![
        MirBasicBlock {
            id: block(owner, 0),
            instructions: vec![MirInstruction::Assign(MirAssignment {
                result: condition,
                rvalue: MirRvalue {
                    kind: MirRvalueKind::ConstantBool(true),
                    ty: MirType::Bool,
                },
                span,
            })],
            terminator: Some(MirTerminator::Branch {
                condition,
                true_target: block(owner, 1),
                false_target: block(owner, 1),
                span,
            }),
            span,
        },
        goto_block(owner, 1, 2, span),
        goto_block(owner, 2, 3, span),
        result,
    ];
    program
}

fn cyclic_program() -> crate::mir::MirProgram {
    let mut program = lower_source_to_final_mir("fn main() -> i64 { return 7; }");
    let definition = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    let owner = definition.callable();
    let span = definition.span;
    definition.body.blocks.extend([
        goto_block(owner, 1, 1, span),
        goto_block(owner, 2, 3, span),
        goto_block(owner, 3, 2, span),
        goto_block(owner, 4, 2, span),
    ]);
    program
}

fn append_unreachable_forwarding_block(body: &mut MirBody) {
    let owner = body.entry.callable();
    let index = body.blocks.len();
    let span = body.blocks[0].span;
    body.blocks.push(goto_block(owner, index, 0, span));
}

fn take_single_entry(definition: &mut MirFunctionDefinition) -> MirBasicBlock {
    assert_eq!(definition.body.blocks.len(), 1);
    definition.body.blocks.pop().unwrap()
}

fn goto_block(
    owner: CallableId,
    index: usize,
    target: usize,
    span: crate::source::Span,
) -> MirBasicBlock {
    MirBasicBlock {
        id: block(owner, index),
        instructions: vec![],
        terminator: Some(MirTerminator::Goto {
            target: block(owner, target),
            span,
        }),
        span,
    }
}

fn block(owner: CallableId, index: usize) -> BlockId {
    BlockId::new(owner, index)
}
