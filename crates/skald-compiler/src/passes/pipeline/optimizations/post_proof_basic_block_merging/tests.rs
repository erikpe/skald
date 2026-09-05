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
use crate::passes::pipeline::{
    optimizations::{post_proof_empty_block_forwarding, whole_world_reachability},
    run_mir_pipeline_measured_inspected,
};

#[test]
fn merges_a_maximal_chain_and_preserves_contents_and_identities() {
    let input = maximal_chain_program();
    let entry = input.entry_function;
    let original = input.definitions.get(entry).unwrap();
    let expected_instructions = original.body.blocks[2].instructions.clone();
    let expected_terminator = original.body.blocks[2].terminator.clone();
    let expected_storage = original.storage.clone();
    let expected_values = original.values.clone();
    let retained_span = original.body.blocks[0].span;
    let moved_instructions = expected_instructions.len() as u64;

    let measured = run_mir_pipeline_with_occurrences(input, &schedule(&[IDENTITY]));
    let output = measured.result.as_ref().unwrap();
    let definition = output.definitions.get(entry).unwrap();
    let record = &measured.occurrences()[0];

    assert_eq!(definition.body.blocks.len(), 1);
    assert_eq!(definition.body.blocks[0].span, retained_span);
    assert_eq!(
        definition.body.blocks[0].instructions,
        expected_instructions
    );
    assert_eq!(definition.body.blocks[0].terminator, expected_terminator);
    assert_eq!(definition.storage, expected_storage);
    assert_eq!(definition.values, expected_values);
    assert_eq!(record.name(), NAME);
    assert_eq!(record.stage(), MirPassStage::Final);
    assert_eq!(record.outcome(), MirPassOccurrenceOutcome::Changed);
    assert_eq!(record.processed_callables(), Some(1));
    assert_eq!(record.changed_callables(), Some(1));
    assert_eq!(
        record.measurements(),
        [
            MirPassMeasurement::count(MERGED_BLOCK_PAIRS, 2),
            MirPassMeasurement::count(MOVED_INSTRUCTIONS, moved_instructions),
            MirPassMeasurement::count(REMOVED_BLOCKS, 2),
            MirPassMeasurement::count(RETAINED_MULTIPLE_INCOMING_EDGE_BARRIERS, 0),
            MirPassMeasurement::count(RETAINED_PERMANENT_ATTACHMENT_BARRIERS, 0),
        ]
    );
    assert_eq!(
        record.removed_mir_entities(),
        Some(measurement(record, REMOVED_BLOCKS))
    );
    assert_eq!(
        output.reachability(),
        &analyze_reachability(output.program()).unwrap()
    );
}

#[test]
fn repeated_occurrence_is_idempotent_and_checkpoints_verified_products() {
    let input = maximal_chain_program();
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
            "after-final-0-post-proof-basic-block-merging-0",
            "after-final-1-post-proof-basic-block-merging-1",
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
        measurement(&measured.occurrences()[1], MERGED_BLOCK_PAIRS),
        0
    );
}

#[test]
fn multiple_incoming_occurrences_are_reported_without_resealing() {
    let input = multiple_incoming_program();
    let expected = verify_final_mir(input.clone()).unwrap();
    let measured = run_mir_pipeline_with_occurrences(input, &schedule(&[IDENTITY]));
    let record = &measured.occurrences()[0];

    assert_eq!(measured.result.as_ref().unwrap(), &expected);
    assert_eq!(measured.statistics.verification_executions(), 2);
    assert_eq!(record.outcome(), MirPassOccurrenceOutcome::Unchanged);
    assert_eq!(measurement(record, MERGED_BLOCK_PAIRS), 0);
    assert_eq!(
        measurement(record, RETAINED_MULTIPLE_INCOMING_EDGE_BARRIERS),
        1
    );
}

#[test]
fn a_two_block_cycle_contracts_deterministically_to_a_self_loop() {
    let input = two_block_cycle_program();
    let entry = input.entry_function;
    let measured = run_mir_pipeline_with_occurrences(input, &schedule(&[IDENTITY]));
    let definition = measured
        .result
        .as_ref()
        .unwrap()
        .definitions
        .get(entry)
        .unwrap();

    assert_eq!(
        measurement(&measured.occurrences()[0], MERGED_BLOCK_PAIRS),
        1
    );
    assert_eq!(definition.body.blocks.len(), 2);
    assert!(matches!(
        definition.body.blocks[1].terminator,
        Some(MirTerminator::Goto { target, .. }) if target == definition.body.blocks[1].id
    ));
}

#[test]
fn functions_members_and_static_initializers_converge_in_one_transaction() {
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
        append_two_block_cycle(&mut input.definitions.get_mut_for_test(function).unwrap().body);
    }
    let members = input
        .member_definitions
        .iter()
        .map(|definition| definition.callable)
        .collect::<Vec<_>>();
    for callable in members {
        append_two_block_cycle(
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
        append_two_block_cycle(&mut initializer.body);
    }
    let callable_count = input.executable_definitions().count();

    let measured = run_mir_pipeline_with_occurrences(input, &schedule(&[IDENTITY]));
    let record = &measured.occurrences()[0];
    assert!(measured.result.is_ok());
    assert_eq!(record.processed_callables(), Some(callable_count as u64));
    assert_eq!(record.changed_callables(), Some(callable_count as u64));
    assert!(measurement(record, MERGED_BLOCK_PAIRS) >= callable_count as u64);
    assert!(
        measurement(record, RETAINED_PERMANENT_ATTACHMENT_BARRIERS) > 0,
        "measurements: {:?}",
        record.measurements()
    );
}

#[test]
fn default_registration_is_selectable_between_forwarding_and_reachability() {
    let default =
        resolve_mir_pass_schedule(MirOptimizationProfile::Default, std::iter::empty()).unwrap();
    let occurrence = default
        .iter()
        .find(|occurrence| occurrence.identity() == IDENTITY)
        .unwrap();
    assert_eq!(occurrence.position(), 11);
    assert_eq!(occurrence.stage(), MirPassStage::Final);
    assert_eq!(
        default.as_slice()[10].identity(),
        post_proof_empty_block_forwarding::IDENTITY
    );
    assert_eq!(
        default.as_slice()[12].identity(),
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

fn maximal_chain_program() -> crate::mir::MirProgram {
    let mut program = lower_source_to_final_mir("fn main() -> i64 { return 7; }");
    let definition = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    let owner = definition.callable();
    let span = definition.span;
    let mut result = take_single_entry(definition);
    result.id = block(owner, 2);
    definition.body.entry = block(owner, 0);
    definition.body.blocks = vec![
        goto_block(owner, 0, 1, span),
        goto_block(owner, 1, 2, span),
        result,
    ];
    program
}

fn multiple_incoming_program() -> crate::mir::MirProgram {
    let mut program = lower_source_to_final_mir("fn main() -> i64 { return 7; }");
    let definition = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    let owner = definition.callable();
    let span = definition.span;
    let mut result = take_single_entry(definition);
    result.id = block(owner, 2);
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
                true_target: block(owner, 2),
                false_target: block(owner, 2),
                span,
            }),
            span,
        },
        goto_block(owner, 1, 2, span),
        result,
    ];
    program
}

fn two_block_cycle_program() -> crate::mir::MirProgram {
    let mut program = lower_source_to_final_mir("fn main() -> i64 { return 7; }");
    let definition = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    append_two_block_cycle(&mut definition.body);
    program
}

fn append_two_block_cycle(body: &mut MirBody) {
    let owner = body.entry.callable();
    let first = body.blocks.len();
    let second = first + 1;
    let span = body.blocks[0].span;
    body.blocks.extend([
        goto_block(owner, first, second, span),
        goto_block(owner, second, first, span),
    ]);
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
