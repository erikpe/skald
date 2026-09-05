use crate::{
    identity::CallableId,
    mir::{rewrite::storage_use_census_for_definition, BlockId, MirBasicBlock, MirTerminator},
    passes::reachability::analyze_reachability,
    test_support::lower_source_to_final_mir,
};

use super::{
    optimizations::{
        constant_short_circuit_folding, post_proof_basic_block_merging,
        post_proof_empty_block_forwarding, post_proof_unreachable_block_elimination,
        whole_world_reachability,
    },
    resolve_exact_mir_pass_schedule, resolve_mir_pass_schedule, run_mir_pipeline_with_occurrences,
    MirOptimizationProfile, MirPassIdentity, MirPassOccurrenceOutcome, MirPassStage,
};

const FINAL_SUFFIX: [MirPassIdentity; 4] = [
    post_proof_unreachable_block_elimination::IDENTITY,
    post_proof_empty_block_forwarding::IDENTITY,
    post_proof_basic_block_merging::IDENTITY,
    whole_world_reachability::IDENTITY,
];
const FINAL_SUFFIX_NAMES: [&str; 4] = [
    "post-proof-unreachable-block-elimination",
    "post-proof-empty-block-forwarding",
    "post-proof-basic-block-merging",
    "whole-world-reachability",
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
fn every_final_suffix_pass_preserves_normalized_activation_storage_semantics() {
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

    for identity in FINAL_SUFFIX {
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
    assert_eq!(measured.statistics.pass_executions(), 9);
    assert_eq!(measured.statistics.normalization_executions(), 1);
    assert!(measured
        .occurrences()
        .iter()
        .all(|record| record.stage() != MirPassStage::Final));
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
