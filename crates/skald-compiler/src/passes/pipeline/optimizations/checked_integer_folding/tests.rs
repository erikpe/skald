use crate::{
    mir::{
        dump_mir,
        rewrite::{rewrite_program, MirProgramRewriteResult},
        MirDefinitionRef, MirInstruction, MirIntegerDivisionKind, MirPathCondition, MirRvalueKind,
        MirShiftDirection, MirStorage, MirStorageKind, MirTerminator, MirType, PathConditionId,
        StorageId,
    },
    passes::{
        resolve_exact_mir_pass_schedule, resolve_mir_pass_schedule,
        run_mir_pipeline_with_occurrences, verify_final_mir, MirOptimizationProfile,
        MirPassMeasurement, MirPassOccurrenceOutcome,
    },
    test_support::lower_source_to_final_mir,
};

use super::*;
use crate::passes::pipeline::optimizations::{
    primitive_constant_folding, primitive_evaluation::PrimitiveConstant,
};
use crate::passes::pipeline::run_mir_pipeline_measured_inspected;

fn division_plan(program: &MirProgram) -> CheckedIntegerFoldPlan {
    CheckedIntegerFoldPlan::prepare(program, CheckedIntegerFoldSelection::DivisionAndRemainder)
        .unwrap()
}

fn shift_plan(program: &MirProgram) -> CheckedIntegerFoldPlan {
    CheckedIntegerFoldPlan::prepare(program, CheckedIntegerFoldSelection::Shift).unwrap()
}

fn all_checked_integer_plan(program: &MirProgram) -> CheckedIntegerFoldPlan {
    CheckedIntegerFoldPlan::prepare(program, CheckedIntegerFoldSelection::All).unwrap()
}

fn apply_plan(program: MirProgram, plan: &CheckedIntegerFoldPlan) -> MirProgramRewriteResult {
    rewrite_program(program, |callable, edit| {
        plan.rewrite_callable(callable, edit).map(|_| ())
    })
    .unwrap()
}

fn apply_and_assert_rewritten(
    program: MirProgram,
    plan: &CheckedIntegerFoldPlan,
) -> MirProgramRewriteResult {
    let result = apply_plan(program, plan);
    for candidate in candidates(plan) {
        let report = result
            .callables
            .iter()
            .find(|report| report.callable == candidate.check_block.callable())
            .unwrap();
        for operand in candidate.operand_loads {
            assert!(report.maps.values.committed(operand.value).is_err());
        }
        let check = report.maps.blocks.committed(candidate.check_block).unwrap();
        let success = report
            .maps
            .blocks
            .committed(candidate.success_block)
            .unwrap();
        let result_value = report
            .maps
            .values
            .committed(candidate.result_assignment.value)
            .unwrap();
        let rewritten = definition(&result.program, candidate.check_block.callable());
        assert!(matches!(
            rewritten.block(check).unwrap().terminator,
            Some(MirTerminator::Goto { target, span })
                if target == success && span == candidate.check_span
        ));
        let MirInstruction::Assign(assignment) = &rewritten.block(success).unwrap().instructions[0]
        else {
            panic!("folded success must begin with the retained result assignment");
        };
        assert_eq!(assignment.result, result_value);
        assert_eq!(
            assignment.rvalue.kind,
            candidate.constant.into_rvalue_kind()
        );
        assert_eq!(assignment.span, candidate.result_assignment.span);
    }
    result
}

fn candidates(plan: &CheckedIntegerFoldPlan) -> Vec<&CheckedIntegerProtocolCandidate> {
    plan.candidates.values().flatten().collect()
}

fn definition(program: &MirProgram, callable: CallableId) -> MirDefinitionRef<'_> {
    program
        .executable_definitions()
        .find(|definition| definition.callable() == callable)
        .unwrap()
}

fn checked_division_count(program: &MirProgram) -> usize {
    program
        .executable_definitions()
        .flat_map(|definition| &definition.body().blocks)
        .filter(|block| {
            matches!(
                block.terminator,
                Some(MirTerminator::IntegerDivisorCheck { .. })
            )
        })
        .count()
}

fn checked_shift_count(program: &MirProgram) -> usize {
    program
        .executable_definitions()
        .flat_map(|definition| &definition.body().blocks)
        .filter(|block| {
            matches!(
                block.terminator,
                Some(MirTerminator::ShiftCountCheck { .. })
            )
        })
        .count()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MirShape {
    blocks: usize,
    instructions: usize,
    values: usize,
    divisor_checks: usize,
    shift_checks: usize,
    checked_operations: usize,
}

fn mir_shape(program: &MirProgram) -> MirShape {
    let definitions = program.executable_definitions().collect::<Vec<_>>();
    MirShape {
        blocks: definitions
            .iter()
            .map(|definition| definition.body().blocks.len())
            .sum(),
        instructions: definitions
            .iter()
            .flat_map(|definition| &definition.body().blocks)
            .map(|block| block.instructions.len())
            .sum(),
        values: definitions
            .iter()
            .map(|definition| definition.values().len())
            .sum(),
        divisor_checks: checked_division_count(program),
        shift_checks: checked_shift_count(program),
        checked_operations: definitions
            .iter()
            .flat_map(|definition| &definition.body().blocks)
            .flat_map(|block| &block.instructions)
            .filter(|instruction| {
                matches!(
                    instruction,
                    MirInstruction::Assign(assignment)
                        if matches!(
                            assignment.rvalue.kind,
                            MirRvalueKind::IntegerDivision { .. } | MirRvalueKind::Shift { .. }
                        )
                )
            })
            .count(),
    }
}

fn fold_ordinary_primitive_constants(program: MirProgram) -> MirProgram {
    let schedule =
        resolve_exact_mir_pass_schedule(&[primitive_constant_folding::IDENTITY]).unwrap();
    run_mir_pipeline_with_occurrences(program, &schedule)
        .result
        .unwrap()
        .program()
        .clone()
}

fn protect_entry_candidate(program: &mut MirProgram, candidate: &CheckedIntegerProtocolCandidate) {
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
        merge: candidate.success_block,
        span: definition.span,
    });
}

#[test]
fn folds_all_quotient_and_remainder_types_in_callable_block_order() {
    let input = lower_source_to_final_mir(concat!(
        "fn div_i64() -> i64 { return 17 / 5; }\n",
        "fn rem_i64() -> i64 { return 17 % 5; }\n",
        "fn div_u64() -> u64 { return 17u / 5u; }\n",
        "fn rem_u64() -> u64 { return 17u % 5u; }\n",
        "fn div_u8() -> u8 { return 17u8 / 5u8; }\n",
        "fn rem_u8() -> u8 { return 17u8 % 5u8; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    let plan = division_plan(&input);
    assert_eq!(plan.candidate_count(), 6);
    assert_eq!(plan.changed_callable_count(), 6);
    assert_eq!(
        candidates(&plan)
            .iter()
            .map(|candidate| candidate.constant)
            .collect::<Vec<_>>(),
        [
            PrimitiveConstant::I64(3),
            PrimitiveConstant::I64(2),
            PrimitiveConstant::U64(3),
            PrimitiveConstant::U64(2),
            PrimitiveConstant::U8(3),
            PrimitiveConstant::U8(2),
        ]
    );

    let result = apply_and_assert_rewritten(input, &plan);
    assert_eq!(checked_division_count(&result.program), 0);
    for candidate in candidates(&plan) {
        let report = result
            .callables
            .iter()
            .find(|report| report.callable == candidate.check_block.callable())
            .unwrap();
        assert_eq!(report.changes.values.removed, 2);
    }
    verify_final_mir(result.program).unwrap();
}

#[test]
fn folds_signed_extrema_without_host_division_behavior() {
    let input = fold_ordinary_primitive_constants(lower_source_to_final_mir(concat!(
        "fn min_quotient() -> i64 { return -9223372036854775808 / -1; }\n",
        "fn min_remainder() -> i64 { return -9223372036854775808 % -1; }\n",
        "fn negative_divisor_quotient() -> i64 { return 17 / -5; }\n",
        "fn negative_divisor_remainder() -> i64 { return 17 % -5; }\n",
        "fn main() -> i64 { return 0; }\n",
    )));
    let plan = division_plan(&input);
    assert_eq!(
        candidates(&plan)
            .iter()
            .map(|candidate| candidate.constant)
            .collect::<Vec<_>>(),
        [
            PrimitiveConstant::I64(i64::MIN),
            PrimitiveConstant::I64(0),
            PrimitiveConstant::I64(-4),
            PrimitiveConstant::I64(-3),
        ]
    );

    let result = apply_plan(input, &plan);
    verify_final_mir(result.program).unwrap();
}

#[test]
fn primitive_folding_exposes_candidates_to_the_checked_protocol_plan() {
    let input = lower_source_to_final_mir("fn main() -> i64 { return (20 + 1) / (2 + 1); }");
    assert!(division_plan(&input).is_empty());
    let primitive_folded = fold_ordinary_primitive_constants(input);
    let plan = division_plan(&primitive_folded);
    assert_eq!(plan.candidate_count(), 1);
    assert_eq!(candidates(&plan)[0].constant, PrimitiveConstant::I64(7));

    let result = apply_plan(primitive_folded, &plan);
    assert_eq!(checked_division_count(&result.program), 0);
    verify_final_mir(result.program).unwrap();
}

#[test]
fn zero_dynamic_and_other_checked_protocols_remain_unchanged() {
    for source in [
        "fn main() -> i64 { return 8 / 0; }",
        concat!(
            "fn effect() -> i64 { return 2; }\n",
            "fn main() -> i64 { return effect() / 2; }\n",
        ),
        concat!(
            "fn effect() -> i64 { return 2; }\n",
            "fn main() -> i64 { return 8 % effect(); }\n",
        ),
        "fn main() -> i64 { return (1 / 0) / 2; }",
        "fn main() -> i64 { return 8 << 2u; }",
    ] {
        let input = lower_source_to_final_mir(source);
        let original = input.clone();
        let plan = division_plan(&input);
        assert!(plan.is_empty());
        assert_eq!(
            input, original,
            "planning must leave rejected MIR untouched"
        );
    }
}

#[test]
fn protected_and_noncanonical_division_protocols_are_not_planned() {
    let mut protected = lower_source_to_final_mir("fn main() -> i64 { return 8 / 2; }");
    let initial_plan = division_plan(&protected);
    let candidate = candidates(&initial_plan)[0].clone();
    protect_entry_candidate(&mut protected, &candidate);
    let protected_original = protected.clone();
    assert!(division_plan(&protected).is_empty());
    assert_eq!(protected, protected_original);

    let mut noncanonical = lower_source_to_final_mir("fn main() -> i64 { return 8 / 2; }");
    let initial_plan = division_plan(&noncanonical);
    let candidate = candidates(&initial_plan)[0].clone();
    let definition = noncanonical
        .definitions
        .get_mut_for_test(noncanonical.entry_function)
        .unwrap();
    let MirInstruction::Assign(assignment) =
        &mut definition.body.blocks[candidate.success_block.index()].instructions[2]
    else {
        unreachable!();
    };
    let MirRvalueKind::IntegerDivision { operation, .. } = &mut assignment.rvalue.kind else {
        unreachable!();
    };
    operation.kind = match operation.kind {
        MirIntegerDivisionKind::Quotient => MirIntegerDivisionKind::Remainder,
        MirIntegerDivisionKind::Remainder => MirIntegerDivisionKind::Quotient,
    };
    let noncanonical_original = noncanonical.clone();
    assert!(division_plan(&noncanonical).is_empty());
    assert_eq!(noncanonical, noncanonical_original);
}

#[test]
fn folding_preserves_operand_assignments_and_carrier_store_order() {
    for (source, selection) in [
        (
            "fn main() -> i64 { return 17 / 5; }",
            CheckedIntegerFoldSelection::DivisionAndRemainder,
        ),
        (
            "fn main() -> i64 { return 3 << 2u; }",
            CheckedIntegerFoldSelection::Shift,
        ),
    ] {
        let input = lower_source_to_final_mir(source);
        let plan = CheckedIntegerFoldPlan::prepare(&input, selection).unwrap();
        let candidate = candidates(&plan)[0];
        let original = definition(&input, candidate.check_block.callable());
        let retained = candidate.operands.map(|operand| {
            (
                original
                    .block(operand.source_assignment.block)
                    .unwrap()
                    .instructions[operand.source_assignment.instruction]
                    .clone(),
                original.block(operand.store.block).unwrap().instructions
                    [operand.store.instruction]
                    .clone(),
            )
        });

        let result = apply_plan(input, &plan);
        let rewritten = definition(&result.program, candidate.check_block.callable());
        for (operand, (assignment, store)) in candidate.operands.iter().zip(retained) {
            assert_eq!(
                rewritten
                    .block(operand.source_assignment.block)
                    .unwrap()
                    .instructions[operand.source_assignment.instruction],
                assignment
            );
            assert_eq!(
                rewritten.block(operand.store.block).unwrap().instructions
                    [operand.store.instruction],
                store
            );
        }
        verify_final_mir(result.program).unwrap();
    }
}

#[test]
fn multiple_and_nested_candidates_compact_once_and_second_plan_is_empty() {
    let input = lower_source_to_final_mir("fn main() -> i64 { return ((8 / 2) + (7 % 3)) / 2; }");
    assert_eq!(checked_division_count(&input), 3);
    let plan = division_plan(&input);
    assert_eq!(plan.candidate_count(), 2);
    assert_eq!(plan.changed_callable_count(), 1);

    let first = apply_plan(input, &plan);
    let report = first
        .callables
        .iter()
        .find(|report| report.callable == first.program.entry_function.into())
        .unwrap();
    assert_eq!(report.changes.values.removed, 4);
    assert_eq!(checked_division_count(&first.program), 1);
    let entry = first
        .program
        .definitions
        .get(first.program.entry_function)
        .unwrap();
    assert!(entry
        .values
        .iter()
        .enumerate()
        .all(|(index, value)| value.id.index() == index));
    assert!(entry
        .body
        .blocks
        .iter()
        .enumerate()
        .all(|(index, block)| block.id.index() == index));
    verify_final_mir(first.program.clone()).unwrap();

    let stable = first.program.clone();
    let second = division_plan(&first.program);
    assert!(second.is_empty());
    assert_eq!(first.program, stable);
}

#[test]
fn mixed_family_rewrite_leaves_shift_callable_byte_for_byte_unchanged() {
    let input = lower_source_to_final_mir(concat!(
        "fn divide() -> i64 { return 8 / 2; }\n",
        "fn shift() -> i64 { return 8 >> 2u; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    let plan = division_plan(&input);
    assert_eq!(plan.candidate_count(), 1);
    let shift_callable = input
        .executable_definitions()
        .find(|definition| {
            definition.body().blocks.iter().any(|block| {
                matches!(
                    block.terminator,
                    Some(MirTerminator::ShiftCountCheck { .. })
                )
            })
        })
        .unwrap()
        .callable();
    let shift_function = shift_callable.as_function().unwrap();
    let original_shift = input.definitions.get(shift_function).unwrap().clone();

    let result = apply_plan(input, &plan);
    assert_eq!(
        result.program.definitions.get(shift_function).unwrap(),
        &original_shift
    );
    verify_final_mir(result.program).unwrap();
}

#[test]
fn folds_every_shift_direction_and_integer_type() {
    let input = lower_source_to_final_mir(concat!(
        "fn shl_i64() -> i64 { return 3 << 2u; }\n",
        "fn shr_i64() -> i64 { return 8 >> 2u; }\n",
        "fn shl_u64() -> u64 { return 3u << 2u; }\n",
        "fn shr_u64() -> u64 { return 8u >> 2u; }\n",
        "fn shl_u8() -> u8 { return 3u8 << 2u; }\n",
        "fn shr_u8() -> u8 { return 8u8 >> 2u; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    let plan = shift_plan(&input);
    assert_eq!(plan, shift_plan(&input));
    assert_eq!(plan.candidate_count(), 6);
    assert_eq!(plan.changed_callable_count(), 6);
    assert_eq!(
        candidates(&plan)
            .iter()
            .map(|candidate| candidate.constant)
            .collect::<Vec<_>>(),
        [
            PrimitiveConstant::I64(12),
            PrimitiveConstant::I64(2),
            PrimitiveConstant::U64(12),
            PrimitiveConstant::U64(2),
            PrimitiveConstant::U8(12),
            PrimitiveConstant::U8(2),
        ]
    );

    let result = apply_and_assert_rewritten(input, &plan);
    assert_eq!(checked_shift_count(&result.program), 0);
    for candidate in candidates(&plan) {
        let report = result
            .callables
            .iter()
            .find(|report| report.callable == candidate.check_block.callable())
            .unwrap();
        assert_eq!(report.changes.values.removed, 2);
    }
    verify_final_mir(result.program).unwrap();
}

#[test]
fn folds_shift_boundaries_with_exact_fixed_width_semantics() {
    let input = fold_ordinary_primitive_constants(lower_source_to_final_mir(concat!(
        "fn zero_count() -> i64 { return -8 >> 0u; }\n",
        "fn signed_max_count() -> i64 { return -8 >> 63u; }\n",
        "fn signed_wrapping_left() -> i64 { return 9223372036854775807 << 1u; }\n",
        "fn signed_arithmetic_right() -> i64 { return -8 >> 2u; }\n",
        "fn unsigned_wrapping_left() -> u64 { return 18446744073709551615u << 1u; }\n",
        "fn unsigned_logical_right() -> u64 { return 9223372036854775808u >> 63u; }\n",
        "fn byte_wrapping_left() -> u8 { return 255u8 << 1u; }\n",
        "fn byte_max_count() -> u8 { return 128u8 >> 7u; }\n",
        "fn main() -> i64 { return 0; }\n",
    )));
    let plan = shift_plan(&input);
    assert_eq!(
        candidates(&plan)
            .iter()
            .map(|candidate| candidate.constant)
            .collect::<Vec<_>>(),
        [
            PrimitiveConstant::I64(-8),
            PrimitiveConstant::I64(-1),
            PrimitiveConstant::I64(-2),
            PrimitiveConstant::I64(-2),
            PrimitiveConstant::U64(u64::MAX - 1),
            PrimitiveConstant::U64(1),
            PrimitiveConstant::U8(254),
            PrimitiveConstant::U8(1),
        ]
    );

    let result = apply_and_assert_rewritten(input, &plan);
    assert_eq!(checked_shift_count(&result.program), 0);
    verify_final_mir(result.program).unwrap();
}

#[test]
fn invalid_or_dynamic_shift_counts_and_operands_remain_unchanged() {
    for source in [
        "fn main() -> i64 { return 8 << 64u; }",
        "fn main() -> i64 { return 8 >> 18446744073709551615u; }",
        "fn shift() -> u8 { return 1u8 << 8u; } fn main() -> i64 { return 0; }",
        concat!(
            "fn shift(value: i64) -> i64 { return value << 2u; }\n",
            "fn main() -> i64 { return 0; }\n",
        ),
        concat!(
            "fn shift(count: u64) -> i64 { return 8 >> count; }\n",
            "fn main() -> i64 { return 0; }\n",
        ),
        concat!(
            "fn effect() -> i64 { return 8; }\n",
            "fn main() -> i64 { return effect() << 2u; }\n",
        ),
        "fn main() -> i64 { return 8 / 2; }",
    ] {
        let input = lower_source_to_final_mir(source);
        let original = input.clone();
        assert!(shift_plan(&input).is_empty());
        assert_eq!(input, original);
    }
}

#[test]
fn protected_and_noncanonical_shift_protocols_are_not_planned() {
    let mut protected = lower_source_to_final_mir("fn main() -> i64 { return 8 >> 2u; }");
    let initial_plan = shift_plan(&protected);
    let candidate = candidates(&initial_plan)[0].clone();
    protect_entry_candidate(&mut protected, &candidate);
    let protected_original = protected.clone();
    assert!(shift_plan(&protected).is_empty());
    assert_eq!(protected, protected_original);

    let mut noncanonical = lower_source_to_final_mir("fn main() -> i64 { return 8 >> 2u; }");
    let initial_plan = shift_plan(&noncanonical);
    let candidate = candidates(&initial_plan)[0].clone();
    let definition = noncanonical
        .definitions
        .get_mut_for_test(noncanonical.entry_function)
        .unwrap();
    let MirInstruction::Assign(assignment) =
        &mut definition.body.blocks[candidate.success_block.index()].instructions[2]
    else {
        unreachable!();
    };
    let MirRvalueKind::Shift { operation, .. } = &mut assignment.rvalue.kind else {
        unreachable!();
    };
    operation.direction = match operation.direction {
        MirShiftDirection::Left => MirShiftDirection::Right,
        MirShiftDirection::Right => MirShiftDirection::Left,
    };
    let noncanonical_original = noncanonical.clone();
    assert!(shift_plan(&noncanonical).is_empty());
    assert_eq!(noncanonical, noncanonical_original);
}

#[test]
fn multiple_shift_candidates_share_one_commit_and_repeat_idempotently() {
    let input = lower_source_to_final_mir("fn main() -> i64 { return (1 << 2u) + (8 >> 1u); }");
    let plan = shift_plan(&input);
    assert_eq!(plan.candidate_count(), 2);
    assert_eq!(plan.changed_callable_count(), 1);

    let first = apply_and_assert_rewritten(input, &plan);
    let report = first
        .callables
        .iter()
        .find(|report| report.callable == first.program.entry_function.into())
        .unwrap();
    assert_eq!(report.changes.values.removed, 4);
    assert_eq!(checked_shift_count(&first.program), 0);
    verify_final_mir(first.program.clone()).unwrap();
    assert!(shift_plan(&first.program).is_empty());
}

#[test]
fn nested_shift_keeps_the_unproven_outer_protocol_checked() {
    let input = lower_source_to_final_mir("fn main() -> i64 { return (1 << 2u) << 1u; }");
    assert_eq!(checked_shift_count(&input), 2);
    let plan = shift_plan(&input);
    assert_eq!(plan.candidate_count(), 1);

    let first = apply_and_assert_rewritten(input, &plan);
    assert_eq!(checked_shift_count(&first.program), 1);
    verify_final_mir(first.program.clone()).unwrap();
    assert!(shift_plan(&first.program).is_empty());
}

#[test]
fn combined_plan_folds_division_and_shift_through_the_same_transaction() {
    let input = lower_source_to_final_mir("fn main() -> i64 { return (8 / 2) + (8 >> 2u); }");
    let plan = all_checked_integer_plan(&input);
    assert_eq!(plan.candidate_count(), 2);
    assert_eq!(plan.changed_callable_count(), 1);

    let result = apply_and_assert_rewritten(input, &plan);
    let report = result
        .callables
        .iter()
        .find(|report| report.callable == result.program.entry_function.into())
        .unwrap();
    assert_eq!(report.changes.values.removed, 4);
    assert_eq!(checked_division_count(&result.program), 0);
    assert_eq!(checked_shift_count(&result.program), 0);
    verify_final_mir(result.program).unwrap();
}

#[test]
fn registered_pass_reports_exact_protocol_and_commit_measurements() {
    let input = lower_source_to_final_mir(concat!(
        "fn quotient() -> i64 { return 8 / 2; }\n",
        "fn remainder() -> i64 { return 7 % 3; }\n",
        "fn shift() -> i64 { return 8 >> 2u; }\n",
        "fn divide_by_zero() -> i64 { return 8 / 0; }\n",
        "fn remainder_by_zero() -> i64 { return 7 % 0; }\n",
        "fn invalid_shift() -> i64 { return 8 << 64u; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    let processed_callables = u64::try_from(input.executable_definitions().count()).unwrap();
    let schedule = resolve_exact_mir_pass_schedule(&[IDENTITY]).unwrap();

    let measured = run_mir_pipeline_with_occurrences(input, &schedule);
    let output = measured.result.as_ref().unwrap();
    let record = &measured.occurrences()[0];

    assert_eq!(record.identity(), IDENTITY);
    assert_eq!(record.name(), NAME);
    assert_eq!(record.outcome(), MirPassOccurrenceOutcome::Changed);
    assert_eq!(record.processed_callables(), Some(processed_callables));
    assert_eq!(record.changed_callables(), Some(3));
    assert_eq!(record.inserted_mir_entities(), Some(0));
    assert_eq!(record.removed_mir_entities(), Some(6));
    assert_eq!(record.verification_executions(), 1);
    let expected_measurements = [
        MirPassMeasurement::count(FOLDED_QUOTIENTS, 1),
        MirPassMeasurement::count(FOLDED_REMAINDERS, 1),
        MirPassMeasurement::count(FOLDED_SHIFTS, 1),
        MirPassMeasurement::count(REMOVED_PROTOCOL_LOAD_VALUES, 6),
        MirPassMeasurement::count(RETAINED_STATIC_FAILURES, 3),
    ];
    assert_eq!(record.measurements(), expected_measurements);
    assert_eq!(
        measured.statistics.pass_measurements().collect::<Vec<_>>(),
        expected_measurements
            .into_iter()
            .map(|measurement| (IDENTITY, NAME, measurement))
            .collect::<Vec<_>>()
    );
    assert_eq!(checked_division_count(output.program()), 2);
    assert_eq!(checked_shift_count(output.program()), 1);
}

#[test]
fn registered_pass_is_unchanged_and_does_not_reverify_without_candidates() {
    let input = lower_source_to_final_mir(concat!(
        "fn divide_by_zero() -> i64 { return 8 / 0; }\n",
        "fn invalid_shift() -> i64 { return 8 << 64u; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    let expected = input.clone();
    let processed_callables = u64::try_from(input.executable_definitions().count()).unwrap();
    let schedule = resolve_exact_mir_pass_schedule(&[IDENTITY]).unwrap();

    let measured = run_mir_pipeline_with_occurrences(input, &schedule);
    let output = measured.result.as_ref().unwrap();
    let record = &measured.occurrences()[0];

    assert_eq!(output.program(), &expected);
    assert_eq!(record.outcome(), MirPassOccurrenceOutcome::Unchanged);
    assert_eq!(record.processed_callables(), Some(processed_callables));
    assert_eq!(record.changed_callables(), Some(0));
    assert_eq!(record.removed_mir_entities(), Some(0));
    assert_eq!(record.verification_executions(), 0);
    assert_eq!(
        record.measurements(),
        [
            MirPassMeasurement::count(FOLDED_QUOTIENTS, 0),
            MirPassMeasurement::count(FOLDED_REMAINDERS, 0),
            MirPassMeasurement::count(FOLDED_SHIFTS, 0),
            MirPassMeasurement::count(REMOVED_PROTOCOL_LOAD_VALUES, 0),
            MirPassMeasurement::count(RETAINED_STATIC_FAILURES, 2),
        ]
    );
}

#[test]
fn repeated_exact_schedule_is_idempotent_and_has_stable_checkpoints() {
    let source = "fn main() -> i64 { return 8 / 2; }";
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
    assert!(inspected.occurrences().is_empty());
    let measured = run_mir_pipeline_with_occurrences(lower_source_to_final_mir(source), &schedule);
    assert!(measured.result.is_ok());
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
        labels,
        [
            "proof-rich-input",
            "after-proof-rich-0-checked-integer-constant-folding-0",
            "after-proof-rich-1-checked-integer-constant-folding-1",
            "after-proof-normalization",
            "final",
        ]
    );
}

#[test]
fn default_schedule_exposes_then_folds_and_cleans_checked_protocols() {
    let source = "fn main() -> i64 { return (6 * 7) / (1 + 1); }";
    let input = lower_source_to_final_mir(source);
    assert!(all_checked_integer_plan(&input).is_empty());

    let default = run_mir_pipeline_with_occurrences(
        input.clone(),
        &resolve_mir_pass_schedule(MirOptimizationProfile::Default, std::iter::empty()).unwrap(),
    );
    let disabled = run_mir_pipeline_with_occurrences(
        input.clone(),
        &resolve_mir_pass_schedule(
            MirOptimizationProfile::Default,
            ["checked-integer-constant-folding"],
        )
        .unwrap(),
    );
    let cfg_disabled = run_mir_pipeline_with_occurrences(
        input.clone(),
        &resolve_mir_pass_schedule(
            MirOptimizationProfile::Default,
            ["conservative-cfg-cleanup"],
        )
        .unwrap(),
    );
    let none = run_mir_pipeline_with_occurrences(
        input.clone(),
        &resolve_mir_pass_schedule(MirOptimizationProfile::None, std::iter::empty()).unwrap(),
    );
    let default_program = default.result.as_ref().unwrap().program();
    let disabled_program = disabled.result.as_ref().unwrap().program();
    let cfg_disabled_program = cfg_disabled.result.as_ref().unwrap().program();

    assert_eq!(checked_division_count(default_program), 0);
    assert_eq!(checked_division_count(disabled_program), 1);
    assert_eq!(checked_division_count(cfg_disabled_program), 0);
    assert_eq!(none.result.as_ref().unwrap().program(), &input);
    assert!(none.occurrences().is_empty());
    let default_blocks = default_program
        .definitions
        .get(default_program.entry_function)
        .unwrap()
        .body
        .blocks
        .len();
    let disabled_blocks = disabled_program
        .definitions
        .get(disabled_program.entry_function)
        .unwrap()
        .body
        .blocks
        .len();
    assert!(default_blocks < disabled_blocks);
    let cfg_disabled_blocks = cfg_disabled_program
        .definitions
        .get(cfg_disabled_program.entry_function)
        .unwrap()
        .body
        .blocks
        .len();
    assert!(default_blocks < cfg_disabled_blocks);

    let checked_record = default
        .occurrences()
        .iter()
        .find(|record| record.identity() == IDENTITY)
        .unwrap();
    assert_eq!(checked_record.position(), 4);
    assert_eq!(checked_record.outcome(), MirPassOccurrenceOutcome::Changed);
    assert_eq!(
        checked_record.measurements()[0],
        MirPassMeasurement::count(FOLDED_QUOTIENTS, 1)
    );
    assert!(default.occurrences().iter().any(|record| {
        record.name() == "conservative-cfg-cleanup"
            && record.outcome() == MirPassOccurrenceOutcome::Changed
            && record.removed_mir_entities().unwrap_or(0) > 0
    }));
    assert!(disabled
        .occurrences()
        .iter()
        .all(|record| record.identity() != IDENTITY));
}

#[test]
fn default_product_has_stable_structural_win_and_backend_input() {
    let source = concat!(
        "fn main() -> i64 {\n",
        "    var quotient: i64 = 84 / 2;\n",
        "    var shifted: i64 = 8 << 2u;\n",
        "    return quotient + shifted;\n",
        "}\n",
    );
    let input = lower_source_to_final_mir(source);
    let default_schedule =
        resolve_mir_pass_schedule(MirOptimizationProfile::Default, std::iter::empty()).unwrap();
    let checked_disabled_schedule = resolve_mir_pass_schedule(
        MirOptimizationProfile::Default,
        ["checked-integer-constant-folding"],
    )
    .unwrap();
    let cfg_disabled_schedule = resolve_mir_pass_schedule(
        MirOptimizationProfile::Default,
        ["conservative-cfg-cleanup"],
    )
    .unwrap();
    let none_schedule =
        resolve_mir_pass_schedule(MirOptimizationProfile::None, std::iter::empty()).unwrap();

    let first = run_mir_pipeline_with_occurrences(input.clone(), &default_schedule);
    let second = run_mir_pipeline_with_occurrences(input.clone(), &default_schedule);
    let checked_disabled =
        run_mir_pipeline_with_occurrences(input.clone(), &checked_disabled_schedule);
    let cfg_disabled = run_mir_pipeline_with_occurrences(input.clone(), &cfg_disabled_schedule);
    let none = run_mir_pipeline_with_occurrences(input.clone(), &none_schedule);
    let first_program = first.result.as_ref().unwrap().program();
    let second_program = second.result.as_ref().unwrap().program();
    let checked_disabled_program = checked_disabled.result.as_ref().unwrap().program();
    let cfg_disabled_program = cfg_disabled.result.as_ref().unwrap().program();
    let none_program = none.result.as_ref().unwrap().program();

    assert_eq!(
        mir_shape(&input),
        MirShape {
            blocks: 7,
            instructions: 41,
            values: 15,
            divisor_checks: 1,
            shift_checks: 1,
            checked_operations: 2,
        }
    );
    assert_eq!(
        mir_shape(first_program),
        MirShape {
            blocks: 5,
            instructions: 37,
            values: 11,
            divisor_checks: 0,
            shift_checks: 0,
            checked_operations: 0,
        }
    );
    assert_eq!(
        mir_shape(cfg_disabled_program),
        MirShape {
            blocks: 7,
            instructions: 37,
            values: 11,
            divisor_checks: 0,
            shift_checks: 0,
            checked_operations: 0,
        }
    );
    assert_eq!(mir_shape(checked_disabled_program), mir_shape(&input));
    assert_eq!(mir_shape(none_program), mir_shape(&input));
    let input_dump = dump_mir(&input);
    let default_dump = dump_mir(first_program);
    assert_eq!(default_dump, dump_mir(second_program));
    assert_eq!(dump_mir(none_program), input_dump);
    assert!(input_dump.contains("integer-divisor-check"));
    assert!(input_dump.contains("shift-count-check"));
    assert!(dump_mir(checked_disabled_program).contains("integer-divisor-check"));
    assert!(dump_mir(checked_disabled_program).contains("shift-count-check"));
    assert!(!default_dump.contains("integer-divisor-check"));
    assert!(!default_dump.contains("shift-count-check"));

    let checked_record = first
        .occurrences()
        .iter()
        .find(|record| record.identity() == IDENTITY)
        .unwrap();
    assert_eq!(
        checked_record.measurements(),
        [
            MirPassMeasurement::count(FOLDED_QUOTIENTS, 1),
            MirPassMeasurement::count(FOLDED_REMAINDERS, 0),
            MirPassMeasurement::count(FOLDED_SHIFTS, 1),
            MirPassMeasurement::count(REMOVED_PROTOCOL_LOAD_VALUES, 4),
            MirPassMeasurement::count(RETAINED_STATIC_FAILURES, 0),
        ]
    );
}
