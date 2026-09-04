use crate::{
    mir::{
        rewrite::{rewrite_program, MirProgramRewriteResult},
        MirDefinitionRef, MirInstruction, MirIntegerDivisionKind, MirPathCondition, MirRvalueKind,
        MirStorage, MirStorageKind, MirTerminator, MirType, PathConditionId, StorageId,
    },
    passes::{
        resolve_exact_mir_pass_schedule, run_mir_pipeline_with_occurrences, verify_final_mir,
    },
    test_support::lower_source_to_final_mir,
};

use super::*;
use crate::passes::pipeline::optimizations::{
    primitive_constant_folding, primitive_evaluation::PrimitiveConstant,
};

fn division_plan(program: &MirProgram) -> CheckedIntegerFoldPlan {
    CheckedIntegerFoldPlan::prepare(program, CheckedIntegerFoldFamily::DivisionAndRemainder)
        .unwrap()
}

fn apply_plan(program: MirProgram, plan: &CheckedIntegerFoldPlan) -> MirProgramRewriteResult {
    rewrite_program(program, |callable, edit| {
        plan.rewrite_callable(callable, edit).map(|_| ())
    })
    .unwrap()
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

fn fold_ordinary_primitive_constants(program: MirProgram) -> MirProgram {
    let schedule =
        resolve_exact_mir_pass_schedule(&[primitive_constant_folding::IDENTITY]).unwrap();
    run_mir_pipeline_with_occurrences(program, &schedule)
        .result
        .unwrap()
        .program()
        .clone()
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

    let result = apply_plan(input, &plan);
    assert_eq!(checked_division_count(&result.program), 0);
    for candidate in candidates(&plan) {
        let report = result
            .callables
            .iter()
            .find(|report| report.callable == candidate.check_block.callable())
            .unwrap();
        assert_eq!(report.changes.values.removed, 2);
        let success = report
            .maps
            .blocks
            .committed(candidate.success_block)
            .unwrap();
        let check = report.maps.blocks.committed(candidate.check_block).unwrap();
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
    let definition = protected
        .definitions
        .get_mut_for_test(protected.entry_function)
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
    let input = lower_source_to_final_mir("fn main() -> i64 { return 17 / 5; }");
    let plan = division_plan(&input);
    let candidate = candidates(&plan)[0];
    let original = definition(&input, candidate.check_block.callable());
    let retained = candidate.operands.map(|operand| {
        (
            original
                .block(operand.source_assignment.block)
                .unwrap()
                .instructions[operand.source_assignment.instruction]
                .clone(),
            original.block(operand.store.block).unwrap().instructions[operand.store.instruction]
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
            rewritten.block(operand.store.block).unwrap().instructions[operand.store.instruction],
            store
        );
    }
    verify_final_mir(result.program).unwrap();
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
