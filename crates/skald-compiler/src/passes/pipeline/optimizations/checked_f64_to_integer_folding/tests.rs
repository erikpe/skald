use crate::{
    identity::CallableId,
    mir::{
        rewrite::{rewrite_program, MirProgramRewriteResult, MirRewriteError},
        test_fixtures::checked_primitive_cast_program,
        MirIntegerType, MirPathCondition, MirProgram, MirStorage, MirStorageKind, MirTerminator,
        MirType, PathConditionId, StorageId,
    },
    passes::{
        resolve_exact_mir_pass_schedule, resolve_mir_pass_schedule,
        run_mir_pipeline_with_occurrences, verify_final_mir, MirOptimizationProfile,
        MirPassMeasurement, MirPassOccurrenceOutcome,
    },
    test_support::lower_source_to_final_mir,
};

use super::*;
use crate::passes::pipeline::{
    optimizations::{
        checked_f64_to_integer_rewrite::CheckedF64ToIntegerProtocolCandidate,
        primitive_evaluation::PrimitiveConstant,
    },
    run_mir_pipeline_measured_inspected,
};

fn fold_plan(program: &MirProgram) -> CheckedF64ToIntegerFoldPlan {
    CheckedF64ToIntegerFoldPlan::prepare(program).unwrap()
}

fn candidates(plan: &CheckedF64ToIntegerFoldPlan) -> Vec<&CheckedF64ToIntegerProtocolCandidate> {
    plan.callables
        .values()
        .flat_map(|callable| &callable.candidates)
        .collect()
}

fn apply_plan(program: MirProgram, plan: &CheckedF64ToIntegerFoldPlan) -> MirProgramRewriteResult {
    rewrite_program(program, |callable, edit| {
        plan.rewrite_callable(callable, edit).map(|_| ())
    })
    .unwrap()
}

fn checked_cast_count(program: &MirProgram) -> usize {
    program
        .executable_definitions()
        .flat_map(|definition| &definition.body().blocks)
        .filter(|block| {
            matches!(
                block.terminator,
                Some(MirTerminator::PrimitiveCastRangeCheck { .. })
            )
        })
        .count()
}

fn protect_candidate(program: &mut MirProgram, candidate: &CheckedF64ToIntegerProtocolCandidate) {
    let definition = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    let activation = StorageId::new(definition.callable(), definition.storage.len());
    definition.storage.push(MirStorage {
        id: activation,
        source: None,
        name: "protocol-proof".to_owned(),
        kind: MirStorageKind::PathCondition,
        ty: MirType::Bool,
        span: definition.span,
    });
    definition.body.path_conditions.push(MirPathCondition {
        id: PathConditionId::new(definition.callable(), 0),
        parent: None,
        activation,
        active_predecessor: candidate.success_block,
        inactive_predecessor: candidate.success_block,
        merge: candidate.join_block,
        span: definition.span,
    });
}

#[test]
fn plans_exact_success_boundaries_for_every_target() {
    for (target, bits, expected_bits, expected) in [
        (
            MirIntegerType::I64,
            0xc3e0_0000_0000_0000,
            i64::MIN as u64,
            PrimitiveConstant::I64(i64::MIN),
        ),
        (
            MirIntegerType::I64,
            0x43df_ffff_ffff_ffff,
            9_223_372_036_854_774_784,
            PrimitiveConstant::I64(9_223_372_036_854_774_784),
        ),
        (
            MirIntegerType::U64,
            0xbfef_ffff_ffff_ffff,
            0,
            PrimitiveConstant::U64(0),
        ),
        (
            MirIntegerType::U64,
            0x43ef_ffff_ffff_ffff,
            18_446_744_073_709_549_568,
            PrimitiveConstant::U64(18_446_744_073_709_549_568),
        ),
        (
            MirIntegerType::U8,
            0x8000_0000_0000_0000,
            0,
            PrimitiveConstant::U8(0),
        ),
        (
            MirIntegerType::U8,
            0x406f_ffff_ffff_ffff,
            255,
            PrimitiveConstant::U8(255),
        ),
    ] {
        let input = checked_primitive_cast_program(bits, target, expected_bits);
        let original = input.clone();
        let plan = fold_plan(&input);
        assert_eq!(
            plan.candidate_count(),
            1,
            "target={target:?} bits={bits:#x}"
        );
        assert_eq!(candidates(&plan)[0].constant, expected);
        assert_eq!(input, original, "planning must be read-only");

        let result = apply_plan(input, &plan);
        assert_eq!(checked_cast_count(&result.program), 0);
        verify_final_mir(result.program).unwrap();
    }
}

#[test]
fn retains_nan_infinity_and_finite_range_failures() {
    for (target, bits) in [
        (MirIntegerType::I64, 0x7ff8_0000_0000_0042),
        (MirIntegerType::I64, 0x7ff0_0000_0000_0000),
        (MirIntegerType::I64, 0x43e0_0000_0000_0000),
        (MirIntegerType::U64, 0xbff0_0000_0000_0000),
        (MirIntegerType::U8, 0x4070_0000_0000_0000),
    ] {
        let input = checked_primitive_cast_program(bits, target, 0);
        let original = input.clone();
        let plan = fold_plan(&input);
        assert!(plan.is_empty());
        assert_eq!(plan.counts().retained_static_failures, 1);
        assert_eq!(input, original);
    }
}

#[test]
fn plans_propagated_sources_and_multiple_nested_or_sibling_casts() {
    let propagated = lower_source_to_final_mir("fn main() -> i64 { return (i64) (1.5 + 2.5); }");
    let plan = fold_plan(&propagated);
    assert_eq!(plan.candidate_count(), 1);
    assert_eq!(plan.counts().propagated_source_folds, 1);
    assert_eq!(candidates(&plan)[0].constant, PrimitiveConstant::I64(4));

    for source in [
        "fn main() -> i64 { return (i64) 3.5 + (i64) 4.5; }",
        "fn main() -> i64 { return (i64) ((f64) ((i64) 3.5)); }",
    ] {
        let input = lower_source_to_final_mir(source);
        let plan = fold_plan(&input);
        assert_eq!(plan.candidate_count(), 2, "{source}");
        assert_eq!(plan.changed_callable_count(), 1);
        let result = apply_plan(input, &plan);
        assert_eq!(checked_cast_count(&result.program), 0);
        verify_final_mir(result.program).unwrap();
    }
}

#[test]
fn protected_and_noncanonical_protocols_are_not_planned() {
    let mut protected =
        checked_primitive_cast_program(0x400e_0000_0000_0000, MirIntegerType::I64, 3);
    let candidate = candidates(&fold_plan(&protected))[0].clone();
    protect_candidate(&mut protected, &candidate);
    assert!(fold_plan(&protected).is_empty());

    let mut malformed =
        checked_primitive_cast_program(0x400e_0000_0000_0000, MirIntegerType::I64, 3);
    let candidate = candidates(&fold_plan(&malformed))[0].clone();
    malformed
        .definitions
        .get_mut_for_test(malformed.entry_function)
        .unwrap()
        .body
        .blocks[candidate.success_block.index()]
    .instructions
    .swap(0, 1);
    assert!(fold_plan(&malformed).is_empty());
}

#[test]
fn complete_plan_rejects_stale_input_before_applying_any_candidate() {
    let input = lower_source_to_final_mir("fn main() -> i64 { return (i64) 3.5 + (i64) 4.5; }");
    let entry: CallableId = input.entry_function.into();
    let plan = fold_plan(&input);
    let first_check = candidates(&plan)[0].check_block;

    let error = rewrite_program(input, |callable, edit| {
        if callable != entry {
            return Ok(());
        }
        edit.rewrite_block_terminator(first_check, |_| None)?;
        let stale_edit = edit.clone();
        let error = plan.rewrite_callable(callable, edit).unwrap_err();
        assert_eq!(edit, &stale_edit);
        Err(error)
    })
    .unwrap_err();

    assert_eq!(
        error,
        MirRewriteError::StaleCallableSnapshot {
            callable: entry,
            subject: "checked floating-to-integer fold plan",
        }
    );
}

#[test]
fn complete_plan_rejects_conflicts_before_applying_any_candidate() {
    let input = lower_source_to_final_mir("fn main() -> i64 { return (i64) 3.5 + (i64) 4.5; }");
    let entry: CallableId = input.entry_function.into();
    let mut plan = fold_plan(&input);
    let duplicate = candidates(&plan)[0].clone();
    plan.callables
        .get_mut(&entry)
        .unwrap()
        .candidates
        .push(duplicate);

    let error = rewrite_program(input, |callable, edit| {
        if callable != entry {
            return Ok(());
        }
        let unchanged = edit.clone();
        let error = plan.rewrite_callable(callable, edit).unwrap_err();
        assert_eq!(edit, &unchanged);
        Err(error)
    })
    .unwrap_err();
    assert_eq!(
        error,
        MirRewriteError::StaleCallableSnapshot {
            callable: entry,
            subject: "checked floating-to-integer fold plan conflicts",
        }
    );
}

#[test]
fn registered_pass_reports_exact_measurements_and_noop_seal_reuse() {
    let input = lower_source_to_final_mir(concat!(
        "fn as_i64() -> i64 { return (i64) -9.6; }\n",
        "fn as_u64() -> u64 { return (u64) 255.9; }\n",
        "fn as_u8() -> u8 { return (u8) 255.9; }\n",
        "fn failure() -> i64 { return (i64) 9223372036854775808.0; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    let schedule = resolve_exact_mir_pass_schedule(&[IDENTITY]).unwrap();
    let measured = run_mir_pipeline_with_occurrences(input, &schedule);
    let record = &measured.occurrences()[0];

    assert_eq!(record.identity(), IDENTITY);
    assert_eq!(record.name(), NAME);
    assert_eq!(record.outcome(), MirPassOccurrenceOutcome::Changed);
    assert_eq!(record.changed_callables(), Some(3));
    assert_eq!(record.inserted_mir_entities(), Some(0));
    assert_eq!(record.removed_mir_entities(), Some(3));
    assert_eq!(record.verification_executions(), 1);
    assert_eq!(
        record.measurements(),
        [
            MirPassMeasurement::count(FOLDED_I64, 1),
            MirPassMeasurement::count(FOLDED_U64, 1),
            MirPassMeasurement::count(FOLDED_U8, 1),
            MirPassMeasurement::count(PROPAGATED_SOURCE_FOLDS, 1),
            MirPassMeasurement::count(REMOVED_PROTOCOL_VALUES, 3),
            MirPassMeasurement::count(RETAINED_STATIC_FAILURES, 1),
        ]
    );

    let no_op = lower_source_to_final_mir(concat!(
        "fn failure() -> i64 { return (i64) 9223372036854775808.0; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    let expected = no_op.clone();
    let measured = run_mir_pipeline_with_occurrences(no_op, &schedule);
    let record = &measured.occurrences()[0];
    assert_eq!(measured.result.as_ref().unwrap().program(), &expected);
    assert_eq!(record.outcome(), MirPassOccurrenceOutcome::Unchanged);
    assert_eq!(record.changed_callables(), Some(0));
    assert_eq!(record.verification_executions(), 0);
    assert_eq!(
        record.measurements()[5],
        MirPassMeasurement::count(RETAINED_STATIC_FAILURES, 1)
    );
}

#[test]
fn repeated_exact_schedule_is_idempotent_and_has_stable_checkpoints() {
    let source = "fn main() -> i64 { return (i64) 3.5; }";
    let schedule = resolve_exact_mir_pass_schedule(&[IDENTITY, IDENTITY]).unwrap();
    let mut labels = Vec::new();
    let mut inspector = |checkpoint: crate::passes::MirPipelineCheckpoint<'_>| {
        labels.push(checkpoint.label().to_string());
    };
    let inspected = run_mir_pipeline_measured_inspected(
        lower_source_to_final_mir(source),
        &schedule,
        Some(&mut inspector),
    );
    assert!(inspected.result.is_ok());
    assert_eq!(
        labels,
        [
            "proof-rich-input",
            "after-proof-rich-0-checked-f64-to-integer-constant-folding-0",
            "after-proof-rich-1-checked-f64-to-integer-constant-folding-1",
            "after-proof-normalization",
            "final",
        ]
    );

    let measured = run_mir_pipeline_with_occurrences(lower_source_to_final_mir(source), &schedule);
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
}

#[test]
fn default_and_disabled_profiles_preserve_selection_and_cleanup_contracts() {
    let source = "fn main() -> i64 { return (i64) (1.5 + 2.5); }";
    let input = lower_source_to_final_mir(source);
    let default = run_mir_pipeline_with_occurrences(
        input.clone(),
        &resolve_mir_pass_schedule(MirOptimizationProfile::Default, std::iter::empty()).unwrap(),
    );
    let disabled = run_mir_pipeline_with_occurrences(
        input.clone(),
        &resolve_mir_pass_schedule(
            MirOptimizationProfile::Default,
            ["checked-f64-to-integer-constant-folding"],
        )
        .unwrap(),
    );
    let none = run_mir_pipeline_with_occurrences(
        input.clone(),
        &resolve_mir_pass_schedule(MirOptimizationProfile::None, std::iter::empty()).unwrap(),
    );

    assert_eq!(
        checked_cast_count(default.result.as_ref().unwrap().program()),
        0
    );
    assert_eq!(
        checked_cast_count(disabled.result.as_ref().unwrap().program()),
        1
    );
    assert_eq!(none.result.as_ref().unwrap().program(), &input);
    assert!(none.occurrences().is_empty());
    let record = default
        .occurrences()
        .iter()
        .find(|record| record.identity() == IDENTITY)
        .unwrap();
    assert_eq!(record.position(), 5);
    assert_eq!(record.outcome(), MirPassOccurrenceOutcome::Changed);
    assert_eq!(
        record.measurements()[3],
        MirPassMeasurement::count(PROPAGATED_SOURCE_FOLDS, 0),
        "the earlier primitive fold materializes the source before this pass"
    );
    assert!(disabled
        .occurrences()
        .iter()
        .all(|record| record.identity() != IDENTITY));
}
