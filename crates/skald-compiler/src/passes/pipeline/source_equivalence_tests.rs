use crate::{
    backend::{emit_assembly, BackendInput, Target},
    mir::{
        rewrite::{
            analyze_basic_block_merging, analyze_empty_block_forwarding,
            final_cfg_facts_for_definition,
        },
        MirProgram, MirTerminator,
    },
    test_support::lower_source_to_final_mir,
};

use super::{
    resolve_exact_mir_pass_schedule, resolve_mir_pass_schedule, run_mir_pipeline_with_occurrences,
    MeasuredMirPipeline, MirOptimizationProfile, MirPassSchedule,
};

const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/golden/optimizations/post_proof_cfg_shape.ska"
));

#[test]
fn source_lowering_reaches_both_passes_and_reduces_backend_input() {
    let input = lower_source_to_final_mir(SOURCE);
    let forwarded_loop = input
        .declarations
        .iter()
        .find(|declaration| declaration.name == "forwarded_loop")
        .unwrap()
        .id;

    let normalized = run_mir_pipeline_with_occurrences(
        input.clone(),
        &resolve_exact_mir_pass_schedule(&[]).unwrap(),
    );
    let normalized_program = program(&normalized);
    let loop_definition = normalized_program.definitions.get(forwarded_loop).unwrap();
    let loop_facts = final_cfg_facts_for_definition(loop_definition.into()).unwrap();
    let forwarding = analyze_empty_block_forwarding(&loop_facts);
    assert!(forwarding.candidates().iter().any(|candidate| {
        forwarding.plan().target_for(candidate.block()) != Some(candidate.direct_target())
    }));

    let forwarding_disabled = default_without(["post-proof-empty-block-forwarding"]);
    let merging_disabled = default_without(["post-proof-basic-block-merging"]);
    let both_disabled = default_without([
        "post-proof-empty-block-forwarding",
        "post-proof-basic-block-merging",
    ]);
    let unreachable_disabled = default_without(["post-proof-unreachable-block-elimination"]);
    let default_schedule = default_without(std::iter::empty::<&str>());

    let without_merging = run_mir_pipeline_with_occurrences(input.clone(), &merging_disabled);
    let has_instruction_bearing_merge =
        program(&without_merging)
            .executable_definitions()
            .any(|definition| {
                let facts = final_cfg_facts_for_definition(definition).unwrap();
                analyze_basic_block_merging(&facts)
                    .first_candidate()
                    .is_some_and(|candidate| {
                        facts
                            .block(candidate.successor())
                            .unwrap()
                            .instruction_count()
                            > 0
                    })
            });
    assert!(has_instruction_bearing_merge);

    let optimized = run_mir_pipeline_with_occurrences(input.clone(), &default_schedule);
    let without_forwarding = run_mir_pipeline_with_occurrences(input.clone(), &forwarding_disabled);
    let without_either = run_mir_pipeline_with_occurrences(input.clone(), &both_disabled);
    let without_unreachable = run_mir_pipeline_with_occurrences(input, &unreachable_disabled);

    assert_eq!(
        pass_measurement(
            &optimized,
            "post-proof-empty-block-forwarding",
            "removed forwarding blocks"
        ),
        3
    );
    assert_eq!(
        pass_measurement(
            &optimized,
            "post-proof-empty-block-forwarding",
            "redirected successor occurrences"
        ),
        4
    );
    assert_eq!(
        pass_measurement(
            &optimized,
            "post-proof-basic-block-merging",
            "merged block pairs"
        ),
        1
    );
    assert_eq!(
        pass_measurement(
            &optimized,
            "post-proof-basic-block-merging",
            "moved instructions"
        ),
        1
    );
    assert_eq!(
        total_block_count(program(&optimized)) + 3,
        total_block_count(program(&without_forwarding))
    );
    assert_eq!(
        total_block_count(program(&optimized)) + 1,
        total_block_count(program(&without_merging))
    );
    assert_eq!(
        total_block_count(program(&optimized)) + 4,
        total_block_count(program(&without_either))
    );
    assert_eq!(
        goto_count(program(&optimized)) + 4,
        goto_count(program(&without_either))
    );
    assert_eq!(program(&optimized), program(&without_unreachable));

    let optimized_assembly = assembly(&optimized);
    let uncanonicalized_assembly = assembly(&without_either);
    assert!(
        assembly_instruction_count(&optimized_assembly)
            < assembly_instruction_count(&uncanonicalized_assembly)
    );
    assert!(optimized_assembly.len() < uncanonicalized_assembly.len());
}

fn default_without<'a>(names: impl IntoIterator<Item = &'a str>) -> MirPassSchedule {
    resolve_mir_pass_schedule(MirOptimizationProfile::Default, names).unwrap()
}

fn pass_measurement(measured: &MeasuredMirPipeline, pass: &str, measurement: &str) -> u64 {
    measured
        .occurrences()
        .iter()
        .find(|record| record.name() == pass)
        .unwrap_or_else(|| panic!("missing `{pass}` occurrence"))
        .measurements()
        .iter()
        .find(|candidate| candidate.name() == measurement)
        .unwrap_or_else(|| panic!("missing `{pass}` measurement `{measurement}`"))
        .value()
}

fn program(measured: &MeasuredMirPipeline) -> &MirProgram {
    measured.result.as_ref().unwrap().program()
}

fn total_block_count(program: &MirProgram) -> usize {
    program
        .executable_definitions()
        .map(|definition| definition.body().blocks.len())
        .sum()
}

fn goto_count(program: &MirProgram) -> usize {
    program
        .executable_definitions()
        .flat_map(|definition| &definition.body().blocks)
        .filter(|block| matches!(block.terminator, Some(MirTerminator::Goto { .. })))
        .count()
}

fn assembly(measured: &MeasuredMirPipeline) -> String {
    emit_assembly(
        Target::X86_64SysV,
        BackendInput::without_runtime_trace(measured.result.as_ref().unwrap()),
    )
    .unwrap()
}

fn assembly_instruction_count(assembly: &str) -> usize {
    assembly
        .lines()
        .filter(|line| {
            let trimmed = line.trim();
            line.starts_with("    ") && !trimmed.is_empty() && !trimmed.starts_with('.')
        })
        .count()
}
