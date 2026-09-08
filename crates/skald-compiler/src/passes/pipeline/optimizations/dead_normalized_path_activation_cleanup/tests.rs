use crate::{
    mir::{dump_mir, MirInstruction, MirPlace, MirRvalueKind, MirStorageKind, StorageId},
    passes::{
        reachability::analyze_reachability, resolve_exact_mir_pass_schedule,
        run_mir_pipeline_with_occurrences, MirPassMeasurement, MirPassOccurrenceOutcome,
    },
    test_support::lower_source_to_final_mir,
};

use super::*;
use crate::passes::pipeline::{
    execution::{
        append_complete_dead_activation, append_declaration_only_dead_activation,
        append_materially_used_activation, SINGLE_DEAD_ACTIVATION_SOURCE,
    },
    normalization::{MirProofNormalizationStatistics, MirProofTransitionPlan},
    run_mir_pipeline_with_transition_and_occurrences_for_test,
    seal::{
        reseal_final_mir, transition_proof_mir, MirProofTransitionError, UnverifiedFinalMirProgram,
    },
    VerifiedFinalMirProgram, VerifiedProofMirProgram,
};

fn exact_schedule(identities: &[crate::passes::MirPassIdentity]) -> crate::passes::MirPassSchedule {
    resolve_exact_mir_pass_schedule(identities).unwrap()
}

fn transition_with_mixed_candidates(
    verified: VerifiedProofMirProgram,
    optional_plan: Option<MirProofTransitionPlan>,
) -> Result<(VerifiedFinalMirProgram, MirProofNormalizationStatistics), MirProofTransitionError> {
    let (verified, statistics) = transition_proof_mir(verified, optional_plan)?;
    let (mut program, authority) = verified.invalidate_for_final_transformation().into_parts();
    let definition = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    append_complete_dead_activation(definition, 0, false);
    append_complete_dead_activation(definition, 1, true);
    append_declaration_only_dead_activation(definition);
    let verified = reseal_final_mir(UnverifiedFinalMirProgram::from_parts(program, authority))
        .map_err(MirProofTransitionError::FinalVerification)?;
    Ok((verified, statistics))
}

fn run_mixed(identities: &[crate::passes::MirPassIdentity]) -> crate::passes::MeasuredMirPipeline {
    run_mir_pipeline_with_transition_and_occurrences_for_test(
        lower_source_to_final_mir(SINGLE_DEAD_ACTIVATION_SOURCE),
        &exact_schedule(identities),
        None,
        transition_with_mixed_candidates,
    )
}

#[test]
fn exact_opt_in_removes_all_candidates_and_reports_stable_metrics() {
    let before = run_mixed(&[]).result.unwrap();
    let before_definition = before.definitions.get(before.entry_function).unwrap();
    let protected_storage = before_definition
        .storage
        .iter()
        .find(|storage| storage.kind == MirStorageKind::NormalizedPathActivation)
        .unwrap()
        .clone();
    let protected_protocol = activation_protocol(before_definition, protected_storage.id);
    let before_instruction_count = instruction_count(before_definition);
    let before_storage_count = before_definition.storage.len();
    let before_value_count = before_definition.values.len();
    let before_constant_producers = before_definition
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter(|instruction| {
            matches!(instruction,
            MirInstruction::Assign(assignment)
                if matches!(assignment.rvalue.kind, MirRvalueKind::ConstantBool(_)))
        })
        .count();
    let producer_spans = before_definition
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match instruction {
            MirInstruction::Assign(assignment)
                if matches!(assignment.rvalue.kind, MirRvalueKind::ConstantBool(false)) =>
            {
                Some(assignment.span)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(!producer_spans.is_empty());

    let measured = run_mixed(&[IDENTITY]);
    let output = measured.result.as_ref().unwrap();
    let definition = output.definitions.get(output.entry_function).unwrap();
    let record = &measured.occurrences()[0];

    assert_eq!(record.name(), NAME);
    assert_eq!(record.stage(), MirPassStage::Final);
    assert_eq!(record.outcome(), MirPassOccurrenceOutcome::Changed);
    assert_eq!(record.processed_callables(), Some(1));
    assert_eq!(record.changed_callables(), Some(1));
    assert_eq!(record.removed_mir_entities(), Some(5));
    assert_eq!(record.inserted_mir_entities(), Some(0));
    assert_eq!(record.verification_executions(), 1);
    assert_eq!(
        record.measurements(),
        [
            MirPassMeasurement::count(INSPECTED_CARRIERS, 4),
            MirPassMeasurement::count(REMOVABLE_CARRIERS, 3),
            MirPassMeasurement::count(PROTECTED_CARRIERS, 1),
            MirPassMeasurement::count(REMOVED_STORAGES, 3),
            MirPassMeasurement::count(REMOVED_LOADS, 2),
            MirPassMeasurement::count(REMOVED_STORES, 2),
            MirPassMeasurement::count(REMOVED_LIFETIME_MARKERS, 4),
            MirPassMeasurement::count(REMOVED_VALUES, 2),
            MirPassMeasurement::count(MAXIMUM_PROTOCOL_SIZE, 4),
        ]
    );
    assert_eq!(definition.storage.len(), before_storage_count - 3);
    assert_eq!(definition.values.len(), before_value_count - 2);
    assert_eq!(instruction_count(definition), before_instruction_count - 8);
    assert_eq!(
        definition
            .body
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter(|instruction| matches!(instruction,
                MirInstruction::Assign(assignment)
                    if matches!(assignment.rvalue.kind, MirRvalueKind::ConstantBool(_))))
            .count(),
        before_constant_producers,
        "cleanup must retain producers which fed removed stores"
    );
    let remaining_activations = definition
        .storage
        .iter()
        .filter(|storage| storage.kind == MirStorageKind::NormalizedPathActivation)
        .collect::<Vec<_>>();
    assert_eq!(remaining_activations, [&protected_storage]);
    assert_eq!(
        activation_protocol(definition, protected_storage.id),
        protected_protocol,
        "the materially loaded activation protocol must remain byte-for-byte present"
    );
    for span in producer_spans {
        assert!(definition
            .body
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .any(|instruction| matches!(instruction,
                MirInstruction::Assign(assignment)
                    if assignment.span == span
                        && matches!(assignment.rvalue.kind, MirRvalueKind::ConstantBool(false)))));
    }
    assert_eq!(
        output.reachability(),
        &analyze_reachability(output.program()).unwrap()
    );
}

#[test]
fn repeated_occurrence_changes_once_then_is_a_deterministic_no_op() {
    let first = run_mixed(&[IDENTITY, IDENTITY]);
    let second = run_mixed(&[IDENTITY, IDENTITY]);
    let first_output = first.result.as_ref().unwrap();
    let second_output = second.result.as_ref().unwrap();

    assert_eq!(
        dump_mir(first_output.program()),
        dump_mir(second_output.program())
    );
    assert_eq!(
        first
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
        first
            .occurrences()
            .iter()
            .map(|record| record.measurements())
            .collect::<Vec<_>>(),
        second
            .occurrences()
            .iter()
            .map(|record| record.measurements())
            .collect::<Vec<_>>()
    );
    assert_eq!(first.occurrences()[1].processed_callables(), Some(1));
    assert_eq!(first.occurrences()[1].changed_callables(), Some(0));
    assert_eq!(
        first.occurrences()[1].measurements(),
        [
            MirPassMeasurement::count(INSPECTED_CARRIERS, 1),
            MirPassMeasurement::count(REMOVABLE_CARRIERS, 0),
            MirPassMeasurement::count(PROTECTED_CARRIERS, 1),
            MirPassMeasurement::count(REMOVED_STORAGES, 0),
            MirPassMeasurement::count(REMOVED_LOADS, 0),
            MirPassMeasurement::count(REMOVED_STORES, 0),
            MirPassMeasurement::count(REMOVED_LIFETIME_MARKERS, 0),
            MirPassMeasurement::count(REMOVED_VALUES, 0),
            MirPassMeasurement::count(MAXIMUM_PROTOCOL_SIZE, 0),
        ]
    );
}

const LARGE_PROTOCOL_PAIRS: usize = 128;
const LARGE_DECLARATION_INTERVAL: usize = 8;

fn transition_with_large_interleaved_protocols(
    verified: VerifiedProofMirProgram,
    optional_plan: Option<MirProofTransitionPlan>,
) -> Result<(VerifiedFinalMirProgram, MirProofNormalizationStatistics), MirProofTransitionError> {
    let (verified, statistics) = transition_proof_mir(verified, optional_plan)?;
    let (mut program, authority) = verified.invalidate_for_final_transformation().into_parts();
    let definition = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    let block_count = definition.body.blocks.len();
    for index in 0..LARGE_PROTOCOL_PAIRS {
        let block = index % block_count;
        append_complete_dead_activation(definition, block, index % 2 == 0);
        append_materially_used_activation(definition, block, index % 2 != 0);
        if index % LARGE_DECLARATION_INTERVAL == 0 {
            append_declaration_only_dead_activation(definition);
        }
    }
    let verified = reseal_final_mir(UnverifiedFinalMirProgram::from_parts(program, authority))
        .map_err(MirProofTransitionError::FinalVerification)?;
    Ok((verified, statistics))
}

fn run_large_interleaved() -> crate::passes::MeasuredMirPipeline {
    run_mir_pipeline_with_transition_and_occurrences_for_test(
        lower_source_to_final_mir(SINGLE_DEAD_ACTIVATION_SOURCE),
        &exact_schedule(&[IDENTITY, IDENTITY]),
        None,
        transition_with_large_interleaved_protocols,
    )
}

#[test]
fn large_interleaved_protocol_set_is_batched_deterministic_and_idempotent() {
    let first = run_large_interleaved();
    let second = run_large_interleaved();
    let first_output = first.result.as_ref().unwrap();
    let second_output = second.result.as_ref().unwrap();
    let declaration_only = LARGE_PROTOCOL_PAIRS / LARGE_DECLARATION_INTERVAL;
    let removable = LARGE_PROTOCOL_PAIRS + declaration_only;
    let protected = LARGE_PROTOCOL_PAIRS + 1;
    let protocol_pair_count = u64::try_from(LARGE_PROTOCOL_PAIRS).unwrap();
    let removable_count = u64::try_from(removable).unwrap();
    let protected_count = u64::try_from(protected).unwrap();

    assert_eq!(
        dump_mir(first_output.program()),
        dump_mir(second_output.program())
    );
    assert_eq!(
        first
            .occurrences()
            .iter()
            .map(|record| (
                record.outcome(),
                record.processed_callables(),
                record.changed_callables(),
                record.verification_executions(),
                record.measurements(),
            ))
            .collect::<Vec<_>>(),
        second
            .occurrences()
            .iter()
            .map(|record| (
                record.outcome(),
                record.processed_callables(),
                record.changed_callables(),
                record.verification_executions(),
                record.measurements(),
            ))
            .collect::<Vec<_>>()
    );
    assert_eq!(first.occurrences().len(), 2);
    let changed = &first.occurrences()[0];
    assert_eq!(changed.outcome(), MirPassOccurrenceOutcome::Changed);
    assert_eq!(changed.processed_callables(), Some(1));
    assert_eq!(changed.changed_callables(), Some(1));
    assert_eq!(changed.verification_executions(), 1);
    assert_eq!(
        changed.measurements(),
        [
            MirPassMeasurement::count(INSPECTED_CARRIERS, removable_count + protected_count,),
            MirPassMeasurement::count(REMOVABLE_CARRIERS, removable_count),
            MirPassMeasurement::count(PROTECTED_CARRIERS, protected_count),
            MirPassMeasurement::count(REMOVED_STORAGES, removable_count),
            MirPassMeasurement::count(REMOVED_LOADS, protocol_pair_count),
            MirPassMeasurement::count(REMOVED_STORES, protocol_pair_count),
            MirPassMeasurement::count(REMOVED_LIFETIME_MARKERS, protocol_pair_count * 2,),
            MirPassMeasurement::count(REMOVED_VALUES, protocol_pair_count),
            MirPassMeasurement::count(MAXIMUM_PROTOCOL_SIZE, 4),
        ]
    );

    let unchanged = &first.occurrences()[1];
    assert_eq!(unchanged.outcome(), MirPassOccurrenceOutcome::Unchanged);
    assert_eq!(unchanged.changed_callables(), Some(0));
    assert_eq!(unchanged.verification_executions(), 0);
    assert_eq!(
        unchanged.measurements()[0],
        MirPassMeasurement::count(INSPECTED_CARRIERS, protected_count)
    );
    assert_eq!(
        unchanged.measurements()[2],
        MirPassMeasurement::count(PROTECTED_CARRIERS, protected_count)
    );
    let remaining_activations = first_output
        .definitions
        .get(first_output.entry_function)
        .unwrap()
        .storage
        .iter()
        .filter(|storage| storage.kind == MirStorageKind::NormalizedPathActivation)
        .count();
    assert_eq!(remaining_activations, protected);
}

#[test]
fn no_candidate_returns_the_existing_verified_program_unchanged() {
    let input = lower_source_to_final_mir(SINGLE_DEAD_ACTIVATION_SOURCE);
    let expected = run_mir_pipeline_with_occurrences(input.clone(), &exact_schedule(&[]))
        .result
        .unwrap();
    let measured = run_mir_pipeline_with_occurrences(input, &exact_schedule(&[IDENTITY]));
    let unchanged = measured.result.as_ref().unwrap();
    let record = &measured.occurrences()[0];

    assert_eq!(unchanged, &expected);
    assert_eq!(record.outcome(), MirPassOccurrenceOutcome::Unchanged);
    assert_eq!(record.verification_executions(), 0);
}

#[test]
fn registration_is_exactly_selectable_and_has_the_frozen_default_position() {
    let exact = exact_schedule(&[IDENTITY]);
    assert_eq!(exact.normalization_position(), 0);
    assert_eq!(exact.len(), 1);
    assert_eq!(exact.as_slice()[0].identity(), IDENTITY);
    assert_eq!(exact.as_slice()[0].name(), NAME);
    assert_eq!(exact.as_slice()[0].stage(), MirPassStage::Final);

    let default = crate::passes::resolve_mir_pass_schedule(
        crate::passes::MirOptimizationProfile::Default,
        std::iter::empty(),
    )
    .unwrap();
    let occurrence = default
        .iter()
        .find(|occurrence| occurrence.identity() == IDENTITY)
        .unwrap();
    assert_eq!(occurrence.position(), 13);
    assert_eq!(occurrence.stage(), MirPassStage::Final);
    assert_eq!(
        default.as_slice()[12].identity(),
        super::super::post_proof_unreachable_block_elimination::IDENTITY
    );
    assert_eq!(
        default.as_slice()[14].identity(),
        super::super::post_proof_empty_block_forwarding::IDENTITY
    );

    let excluded = crate::passes::resolve_mir_pass_schedule(
        crate::passes::MirOptimizationProfile::Default,
        [NAME],
    )
    .unwrap();
    assert_eq!(excluded.len(), default.len() - 1);
    assert!(excluded
        .iter()
        .all(|occurrence| occurrence.identity() != IDENTITY));
}

fn instruction_count(definition: &crate::mir::MirFunctionDefinition) -> usize {
    definition
        .body
        .blocks
        .iter()
        .map(|block| block.instructions.len())
        .sum()
}

fn activation_protocol(
    definition: &crate::mir::MirFunctionDefinition,
    storage: StorageId,
) -> Vec<MirInstruction> {
    let place = MirPlace::base(storage);
    definition
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter(|instruction| match instruction {
            MirInstruction::StorageLive(operation) => operation.storage == storage,
            MirInstruction::StorageDead(operation) => operation.storage == storage,
            MirInstruction::Store(operation) => operation.destination == place,
            MirInstruction::Assign(assignment) => {
                matches!(&assignment.rvalue.kind, MirRvalueKind::Load(source) if source == &place)
            }
            _ => false,
        })
        .cloned()
        .collect()
}
