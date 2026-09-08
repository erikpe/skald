use crate::{
    identity::CallableId,
    mir::{
        rewrite::storage_use_census_for_definition,
        test_fixtures::{assign, storage_dead, storage_live, store, value},
        BlockId, MirBasicBlock, MirPlace, MirRvalueKind, MirStorageKind, MirTerminator, MirType,
        ValueId,
    },
    passes::{analyze_dead_normalized_path_activations, reachability::analyze_reachability},
    test_support::lower_source_to_final_mir,
};

use super::{
    execution::{append_complete_dead_activation, append_declaration_only_dead_activation},
    normalization::{MirProofNormalizationStatistics, MirProofTransitionPlan},
    optimizations::{
        constant_short_circuit_folding, dead_normalized_path_activation_cleanup,
        post_proof_basic_block_merging, post_proof_empty_block_forwarding,
        post_proof_unreachable_block_elimination, whole_world_reachability,
    },
    resolve_exact_mir_pass_schedule, resolve_mir_pass_schedule, run_mir_pipeline_with_occurrences,
    run_mir_pipeline_with_transition_and_occurrences_for_test,
    seal::{
        reseal_final_mir, transition_proof_mir, MirProofTransitionError, UnverifiedFinalMirProgram,
    },
    MirOptimizationProfile, MirPassIdentity, MirPassOccurrenceOutcome, MirPassStage,
    VerifiedFinalMirProgram, VerifiedProofMirProgram,
};

const FINAL_SUFFIX: [MirPassIdentity; 5] = [
    post_proof_unreachable_block_elimination::IDENTITY,
    dead_normalized_path_activation_cleanup::IDENTITY,
    post_proof_empty_block_forwarding::IDENTITY,
    post_proof_basic_block_merging::IDENTITY,
    whole_world_reachability::IDENTITY,
];
const FINAL_SUFFIX_NAMES: [&str; 5] = [
    "post-proof-unreachable-block-elimination",
    "dead-normalized-path-activation-cleanup",
    "post-proof-empty-block-forwarding",
    "post-proof-basic-block-merging",
    "whole-world-reachability",
];
const STORAGE_PRESERVING_FINAL_PASSES: [MirPassIdentity; 4] = [
    post_proof_unreachable_block_elimination::IDENTITY,
    post_proof_empty_block_forwarding::IDENTITY,
    post_proof_basic_block_merging::IDENTITY,
    whole_world_reachability::IDENTITY,
];

#[test]
fn every_final_suffix_pass_executes_alone_through_the_mandatory_boundary() {
    for identity in FINAL_SUFFIX {
        let measured = run_mir_pipeline_with_occurrences(
            lower_source_to_final_mir("fn main() -> i64 { return 0; }"),
            &exact(&[identity]),
        );

        assert!(measured.result.is_ok());
        assert_eq!(measured.statistics.normalization_executions(), 1);
        assert_eq!(measured.statistics.pass_executions(), 1);
        assert_eq!(measured.occurrences().len(), 1);
        assert_eq!(measured.occurrences()[0].identity(), identity);
        assert_eq!(measured.occurrences()[0].stage(), MirPassStage::Final);
        assert_eq!(
            measured.occurrences()[0].outcome(),
            MirPassOccurrenceOutcome::Unchanged
        );
        assert_eq!(measured.occurrences()[0].verification_executions(), 0);
    }
}

#[test]
fn every_non_storage_final_pass_preserves_normalized_activation_storage_semantics() {
    let input = lower_source_to_final_mir(
        "fn unused() -> bool { return false && true; }
         fn main() -> i64 {
           if (false && true) { return 1; }
           return 0;
         }",
    );
    let unused = input
        .declarations
        .iter()
        .find(|declaration| declaration.name == "unused")
        .unwrap()
        .id;
    let reference = run_mir_pipeline_with_occurrences(
        input.clone(),
        &exact(&[constant_short_circuit_folding::IDENTITY]),
    )
    .result
    .unwrap();
    let expected_storage = reference
        .definitions
        .get(reference.entry_function)
        .unwrap()
        .storage
        .clone();
    let activations = expected_storage
        .iter()
        .filter(|storage| storage.kind.is_normalized_path_activation())
        .map(|storage| storage.id)
        .collect::<Vec<_>>();
    assert!(!activations.is_empty());

    for identity in STORAGE_PRESERVING_FINAL_PASSES {
        let measured = run_mir_pipeline_with_occurrences(
            input.clone(),
            &exact(&[constant_short_circuit_folding::IDENTITY, identity]),
        );
        let final_occurrence = measured.occurrences().last().unwrap();
        assert_eq!(final_occurrence.identity(), identity);
        assert_eq!(
            final_occurrence.verification_executions(),
            u64::from(final_occurrence.outcome() == MirPassOccurrenceOutcome::Changed)
        );
        let output = measured.result.unwrap();
        let definition = output.definitions.get(output.entry_function).unwrap();

        assert_eq!(definition.storage, expected_storage, "{identity:?}");
        let census = storage_use_census_for_definition(definition.into()).unwrap();
        for activation in &activations {
            assert!(
                census
                    .get(*activation)
                    .unwrap()
                    .kind()
                    .is_normalized_path_activation(),
                "{identity:?}"
            );
        }
        if identity == whole_world_reachability::IDENTITY {
            assert!(output.definitions.get(unused).is_none());
        }
    }
}

#[test]
fn disabling_the_complete_final_suffix_retains_the_proof_prefix_and_boundary() {
    let measured = run_mir_pipeline_with_occurrences(
        lower_source_to_final_mir("fn main() -> i64 { return 1 + 0; }"),
        &resolve_mir_pass_schedule(MirOptimizationProfile::Default, FINAL_SUFFIX_NAMES).unwrap(),
    );

    assert!(measured.result.is_ok());
    assert_eq!(measured.statistics.pass_executions(), 12);
    assert_eq!(measured.statistics.normalization_executions(), 1);
    assert!(measured
        .occurrences()
        .iter()
        .all(|record| record.stage() != MirPassStage::Final));
}

#[test]
fn unreachable_deletion_exposes_a_dead_activation_to_default_cleanup() {
    let input = lower_source_to_final_mir("fn main() -> i64 { return 7; }");
    let none = run_mir_pipeline_with_transition_and_occurrences_for_test(
        input.clone(),
        &resolve_mir_pass_schedule(MirOptimizationProfile::None, std::iter::empty()).unwrap(),
        None,
        transition_with_unreachable_material_activation_use,
    );
    let without_unreachable = run_mir_pipeline_with_transition_and_occurrences_for_test(
        input.clone(),
        &exact(&[dead_normalized_path_activation_cleanup::IDENTITY]),
        None,
        transition_with_unreachable_material_activation_use,
    );
    let cleanup_disabled = run_mir_pipeline_with_transition_and_occurrences_for_test(
        input.clone(),
        &exact(&[post_proof_unreachable_block_elimination::IDENTITY]),
        None,
        transition_with_unreachable_material_activation_use,
    );
    let composed = run_mir_pipeline_with_transition_and_occurrences_for_test(
        input,
        &exact(&[
            post_proof_unreachable_block_elimination::IDENTITY,
            dead_normalized_path_activation_cleanup::IDENTITY,
        ]),
        None,
        transition_with_unreachable_material_activation_use,
    );

    assert!(none.occurrences().is_empty());
    assert_eq!(none.statistics.normalization_executions(), 1);
    assert_eq!(activation_storage_count(none.result.as_ref().unwrap()), 1);

    let without_unreachable_output = without_unreachable.result.as_ref().unwrap();
    assert_eq!(
        without_unreachable.occurrences()[0].outcome(),
        MirPassOccurrenceOutcome::Unchanged
    );
    assert_eq!(activation_storage_count(without_unreachable_output), 1);
    assert_eq!(
        measurement(
            &without_unreachable.occurrences()[0],
            "protected normalized activation carriers"
        ),
        1
    );

    let cleanup_disabled_output = cleanup_disabled.result.as_ref().unwrap();
    assert_eq!(activation_storage_count(cleanup_disabled_output), 1);
    assert_eq!(
        analyze_dead_normalized_path_activations(cleanup_disabled_output)
            .counts()
            .proven(),
        1
    );

    let composed_output = composed.result.as_ref().unwrap();
    assert_eq!(
        composed.occurrences()[1].outcome(),
        MirPassOccurrenceOutcome::Changed
    );
    assert_eq!(activation_storage_count(composed_output), 0);
    assert_eq!(
        measurement(&composed.occurrences()[1], "removed storage declarations"),
        1
    );
}

#[test]
fn cleanup_exposes_an_empty_block_to_later_cfg_canonicalization() {
    let input = lower_source_to_final_mir("fn main() -> i64 { return 7; }");
    let forwarding_only = run_mir_pipeline_with_transition_and_occurrences_for_test(
        input.clone(),
        &exact(&[post_proof_empty_block_forwarding::IDENTITY]),
        None,
        transition_with_dead_activation_block,
    );
    let cleanup_only = run_mir_pipeline_with_transition_and_occurrences_for_test(
        input.clone(),
        &exact(&[dead_normalized_path_activation_cleanup::IDENTITY]),
        None,
        transition_with_dead_activation_block,
    );
    let composed = run_mir_pipeline_with_transition_and_occurrences_for_test(
        input,
        &exact(&[
            dead_normalized_path_activation_cleanup::IDENTITY,
            post_proof_empty_block_forwarding::IDENTITY,
            post_proof_basic_block_merging::IDENTITY,
        ]),
        None,
        transition_with_dead_activation_block,
    );

    assert!(
        forwarding_only.result.is_ok(),
        "constructed forwarding fixture must verify: {:?}",
        forwarding_only.result
    );
    assert_eq!(
        occurrence_outcomes(&forwarding_only),
        [MirPassOccurrenceOutcome::Unchanged]
    );
    assert_eq!(main_block_count(&forwarding_only), 3);
    assert_eq!(
        activation_storage_count(forwarding_only.result.as_ref().unwrap()),
        1
    );

    assert_eq!(
        occurrence_outcomes(&cleanup_only),
        [MirPassOccurrenceOutcome::Changed]
    );
    assert_eq!(main_block_count(&cleanup_only), 3);
    assert_eq!(
        activation_storage_count(cleanup_only.result.as_ref().unwrap()),
        0
    );
    assert!(cleanup_only
        .result
        .as_ref()
        .unwrap()
        .definitions
        .get(cleanup_only.result.as_ref().unwrap().entry_function)
        .unwrap()
        .body
        .blocks[1]
        .instructions
        .is_empty());

    assert_eq!(
        occurrence_outcomes(&composed),
        [
            MirPassOccurrenceOutcome::Changed,
            MirPassOccurrenceOutcome::Changed,
            MirPassOccurrenceOutcome::Changed,
        ]
    );
    assert_eq!(main_block_count(&composed), 1);
    assert_eq!(
        activation_storage_count(composed.result.as_ref().unwrap()),
        0
    );
}

#[test]
fn forwarding_exposes_merging_and_the_ordered_pair_reaches_a_fixed_point() {
    let input = forwarding_exposes_merge_program();

    let merging_only = run_mir_pipeline_with_occurrences(
        input.clone(),
        &exact(&[post_proof_basic_block_merging::IDENTITY]),
    );
    assert_eq!(main_block_count(&merging_only), 3);
    assert_eq!(
        occurrence_outcomes(&merging_only),
        [MirPassOccurrenceOutcome::Unchanged]
    );

    let reversed = run_mir_pipeline_with_occurrences(
        input.clone(),
        &exact(&[
            post_proof_basic_block_merging::IDENTITY,
            post_proof_empty_block_forwarding::IDENTITY,
        ]),
    );
    assert_eq!(main_block_count(&reversed), 2);
    assert_eq!(
        occurrence_outcomes(&reversed),
        [
            MirPassOccurrenceOutcome::Unchanged,
            MirPassOccurrenceOutcome::Changed,
        ]
    );

    let converged = run_mir_pipeline_with_occurrences(
        input,
        &exact(&[
            post_proof_empty_block_forwarding::IDENTITY,
            post_proof_basic_block_merging::IDENTITY,
            post_proof_empty_block_forwarding::IDENTITY,
            post_proof_basic_block_merging::IDENTITY,
        ]),
    );
    assert_eq!(main_block_count(&converged), 1);
    assert_eq!(
        occurrence_outcomes(&converged),
        [
            MirPassOccurrenceOutcome::Changed,
            MirPassOccurrenceOutcome::Changed,
            MirPassOccurrenceOutcome::Unchanged,
            MirPassOccurrenceOutcome::Unchanged,
        ]
    );
    assert_eq!(
        converged
            .occurrences()
            .iter()
            .map(|record| record.verification_executions())
            .collect::<Vec<_>>(),
        [1, 1, 0, 0]
    );
    assert_eq!(converged.statistics.verification_executions(), 4);

    let output = converged.result.as_ref().unwrap();
    assert_eq!(
        output.reachability(),
        &analyze_reachability(output.program()).unwrap()
    );
}

#[test]
fn whole_world_reachability_consumes_the_call_graph_after_cfg_canonicalization() {
    let input = lower_source_to_final_mir(
        "fn selected() -> bool { return true; }
         fn main() -> i64 {
           if (true) { return 1; }
           if (false && selected()) { return 2; }
           return 3;
         }",
    );
    let selected = input
        .declarations
        .iter()
        .find(|declaration| declaration.name == "selected")
        .unwrap()
        .id;

    let canonicalized = run_mir_pipeline_with_occurrences(
        input.clone(),
        &resolve_mir_pass_schedule(
            MirOptimizationProfile::Default,
            ["whole-world-reachability"],
        )
        .unwrap(),
    );
    let canonicalized_program = canonicalized.result.as_ref().unwrap();
    assert!(canonicalized_program.definitions.get(selected).is_some());
    assert!(!canonicalized_program
        .reachability()
        .reachable_callables()
        .contains(&CallableId::Function(selected)));

    let retained = run_mir_pipeline_with_occurrences(
        input,
        &resolve_mir_pass_schedule(MirOptimizationProfile::Default, std::iter::empty()).unwrap(),
    );
    let retained_program = retained.result.as_ref().unwrap();
    assert!(retained_program.definitions.get(selected).is_none());
    assert_eq!(
        canonicalized.occurrences().last().unwrap().identity(),
        post_proof_basic_block_merging::IDENTITY
    );
    assert_eq!(
        retained.occurrences().last().unwrap().identity(),
        whole_world_reachability::IDENTITY
    );
    assert_eq!(
        retained_program.reachability(),
        &analyze_reachability(retained_program.program()).unwrap()
    );
}

fn exact(identities: &[MirPassIdentity]) -> super::MirPassSchedule {
    resolve_exact_mir_pass_schedule(identities).unwrap()
}

fn occurrence_outcomes(measured: &super::MeasuredMirPipeline) -> Vec<MirPassOccurrenceOutcome> {
    measured
        .occurrences()
        .iter()
        .map(|record| record.outcome())
        .collect()
}

fn main_block_count(measured: &super::MeasuredMirPipeline) -> usize {
    let program = measured.result.as_ref().unwrap();
    program
        .definitions
        .get(program.entry_function)
        .unwrap()
        .body
        .blocks
        .len()
}

fn activation_storage_count(program: &VerifiedFinalMirProgram) -> usize {
    program
        .executable_definitions()
        .flat_map(|definition| definition.storage_entries())
        .filter(|storage| storage.kind == MirStorageKind::NormalizedPathActivation)
        .count()
}

fn measurement(record: &super::MirPassOccurrenceRecord, name: &str) -> u64 {
    record
        .measurements()
        .iter()
        .find(|measurement| measurement.name() == name)
        .map(|measurement| measurement.value())
        .unwrap_or_else(|| panic!("missing `{name}` measurement"))
}

fn transition_with_dead_activation_block(
    verified: VerifiedProofMirProgram,
    optional_plan: Option<MirProofTransitionPlan>,
) -> Result<(VerifiedFinalMirProgram, MirProofNormalizationStatistics), MirProofTransitionError> {
    let (verified, statistics) = transition_proof_mir(verified, optional_plan)?;
    let (mut program, authority) = verified.invalidate_for_final_transformation().into_parts();
    let definition = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    let owner = definition.callable();
    let span = definition.span;
    let mut entry_block = definition.body.blocks.pop().unwrap();
    assert!(definition.body.blocks.is_empty());
    entry_block.id = BlockId::new(owner, 0);
    entry_block.terminator = Some(MirTerminator::Goto {
        target: BlockId::new(owner, 1),
        span,
    });
    definition.body.entry = BlockId::new(owner, 0);
    definition.body.blocks = vec![
        entry_block,
        goto_block(owner, 1, 2, span),
        MirBasicBlock {
            id: BlockId::new(owner, 2),
            instructions: vec![],
            terminator: None,
            span,
        },
    ];
    let activation = append_declaration_only_dead_activation(definition);
    definition.body.blocks[1].instructions.extend([
        storage_live(activation, span),
        storage_dead(activation, span),
    ]);
    append_i64_return(definition, 2, 7);

    let verified = reseal_final_mir(UnverifiedFinalMirProgram::from_parts(program, authority))
        .map_err(MirProofTransitionError::FinalVerification)?;
    Ok((verified, statistics))
}

fn transition_with_unreachable_material_activation_use(
    verified: VerifiedProofMirProgram,
    optional_plan: Option<MirProofTransitionPlan>,
) -> Result<(VerifiedFinalMirProgram, MirProofNormalizationStatistics), MirProofTransitionError> {
    let (verified, statistics) = transition_proof_mir(verified, optional_plan)?;
    let (mut program, authority) = verified.invalidate_for_final_transformation().into_parts();
    let definition = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    let owner = definition.callable();
    let span = definition.span;
    let mut entry_block = definition.body.blocks.pop().unwrap();
    assert!(definition.body.blocks.is_empty());
    entry_block.id = BlockId::new(owner, 0);
    entry_block.terminator = Some(MirTerminator::Goto {
        target: BlockId::new(owner, 1),
        span,
    });
    definition.body.entry = BlockId::new(owner, 0);
    definition.body.blocks = vec![
        entry_block,
        MirBasicBlock {
            id: BlockId::new(owner, 1),
            instructions: vec![],
            terminator: None,
            span,
        },
        goto_block(owner, 2, 1, span),
    ];

    let activation = append_complete_dead_activation(definition, 0, true);
    append_i64_return(definition, 1, 7);
    let material_load = ValueId::new(owner, definition.values.len());
    definition
        .values
        .push(value(material_load, MirType::Bool, span));
    definition.body.blocks[2].instructions.extend([
        storage_live(activation, span),
        assign(
            material_load,
            MirRvalueKind::Load(MirPlace::base(activation)),
            MirType::Bool,
            span,
        ),
        store(MirPlace::base(activation), material_load, span),
        storage_dead(activation, span),
    ]);

    let verified = reseal_final_mir(UnverifiedFinalMirProgram::from_parts(program, authority))
        .map_err(MirProofTransitionError::FinalVerification)?;
    Ok((verified, statistics))
}

fn append_i64_return(
    definition: &mut crate::mir::MirFunctionDefinition,
    block_index: usize,
    constant: i64,
) {
    let owner = definition.callable();
    let span = definition.body.blocks[block_index].span;
    let result = ValueId::new(owner, definition.values.len());
    definition.values.push(value(result, MirType::I64, span));
    definition.body.blocks[block_index]
        .instructions
        .push(assign(
            result,
            MirRvalueKind::ConstantI64(constant),
            MirType::I64,
            span,
        ));
    definition.body.blocks[block_index].terminator = Some(MirTerminator::Return {
        value: Some(result),
        span,
    });
}

/// The unreachable empty block contributes a second incoming edge to the
/// result block. Merging alone must retain that join; forwarding removes the
/// empty predecessor, after which a fresh CFG snapshot permits entry/result
/// merging.
fn forwarding_exposes_merge_program() -> crate::mir::MirProgram {
    let mut program = lower_source_to_final_mir("fn main() -> i64 { return 7; }");
    let definition = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    let owner = definition.callable();
    let span = definition.span;
    let mut result = definition.body.blocks.pop().unwrap();
    result.id = BlockId::new(owner, 2);
    definition.body.entry = BlockId::new(owner, 0);
    definition.body.blocks = vec![
        goto_block(owner, 0, 2, span),
        goto_block(owner, 1, 2, span),
        result,
    ];
    program
}

fn goto_block(
    owner: CallableId,
    index: usize,
    target: usize,
    span: crate::source::Span,
) -> MirBasicBlock {
    MirBasicBlock {
        id: BlockId::new(owner, index),
        instructions: vec![],
        terminator: Some(MirTerminator::Goto {
            target: BlockId::new(owner, target),
            span,
        }),
        span,
    }
}
