use crate::{
    identity::FunctionId,
    mir::{
        BlockId, MirBasicBlock, MirInstruction, MirPathCondition, MirRvalueKind, MirStorage,
        MirStorageKind, MirTerminationReason, MirTerminator, MirType, PathConditionId, StorageId,
    },
    passes::{resolve_exact_mir_pass_schedule, run_mir_pipeline_with_occurrences},
    test_support::lower_source_to_final_mir,
};

use super::*;
use crate::passes::pipeline::optimizations::primitive_constant_folding;

fn observations(program: &crate::mir::MirProgram) -> Vec<CheckedIntegerProtocolObservation> {
    program
        .executable_definitions()
        .flat_map(|definition| observe_checked_integer_protocols(definition).unwrap())
        .collect()
}

fn only_observation(program: &crate::mir::MirProgram) -> CheckedIntegerProtocolObservation {
    let observations = observations(program);
    assert_eq!(observations.len(), 1, "{observations:#?}");
    observations.into_iter().next().unwrap()
}

fn only_candidate(program: &crate::mir::MirProgram) -> CheckedIntegerProtocolCandidate {
    let CheckedIntegerProtocolObservation::Candidate(candidate) = only_observation(program) else {
        panic!("expected candidate");
    };
    *candidate
}

fn entry_definition_mut(
    program: &mut crate::mir::MirProgram,
) -> &mut crate::mir::MirFunctionDefinition {
    program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap()
}

#[test]
fn discovers_every_division_remainder_and_shift_variant() {
    let program = lower_source_to_final_mir(concat!(
        "fn div_i64() -> i64 { return 17 / 5; }\n",
        "fn rem_i64() -> i64 { return 17 % 5; }\n",
        "fn div_u64() -> u64 { return 17u / 5u; }\n",
        "fn rem_u64() -> u64 { return 17u % 5u; }\n",
        "fn div_u8() -> u8 { return 17u8 / 5u8; }\n",
        "fn rem_u8() -> u8 { return 17u8 % 5u8; }\n",
        "fn shl_i64() -> i64 { return 3 << 2u; }\n",
        "fn shr_i64() -> i64 { return 8 >> 2u; }\n",
        "fn shl_u64() -> u64 { return 3u << 2u; }\n",
        "fn shr_u64() -> u64 { return 8u >> 2u; }\n",
        "fn shl_u8() -> u8 { return 3u8 << 2u; }\n",
        "fn shr_u8() -> u8 { return 8u8 >> 2u; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    let candidates = observations(&program)
        .into_iter()
        .map(|observation| match observation {
            CheckedIntegerProtocolObservation::Candidate(candidate) => candidate,
            rejected => panic!("unexpected rejection: {rejected:#?}"),
        })
        .collect::<Vec<_>>();

    assert_eq!(candidates.len(), 12);
    assert_eq!(
        candidates
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
            PrimitiveConstant::I64(12),
            PrimitiveConstant::I64(2),
            PrimitiveConstant::U64(12),
            PrimitiveConstant::U64(2),
            PrimitiveConstant::U8(12),
            PrimitiveConstant::U8(2),
        ]
    );
    assert!(candidates[..6]
        .iter()
        .all(|candidate| matches!(candidate.check, CheckedIntegerProtocolCheck::Division(_))));
    assert!(candidates[6..]
        .iter()
        .all(|candidate| matches!(candidate.check, CheckedIntegerProtocolCheck::Shift(_))));
}

#[test]
fn candidate_records_the_complete_rewrite_snapshot_and_source_spans() {
    let program = lower_source_to_final_mir("fn main() -> i64 { return 17 / 5; }");
    let original = program.clone();
    let candidate = only_candidate(&program);
    let definition = program.definitions.get(program.entry_function).unwrap();

    assert_eq!(
        program, original,
        "discovery must not mutate or clone-replace MIR"
    );
    assert_eq!(candidate.check_block.callable(), definition.callable());
    assert_eq!(
        candidate.operands[0].storage,
        match candidate.check {
            CheckedIntegerProtocolCheck::Division(check) => check.dividend,
            CheckedIntegerProtocolCheck::Shift(_) => unreachable!(),
        }
    );
    assert_eq!(candidate.operands[0].constant, PrimitiveConstant::I64(17));
    assert_eq!(candidate.operands[1].constant, PrimitiveConstant::I64(5));
    assert_eq!(candidate.constant, PrimitiveConstant::I64(3));
    assert_eq!(candidate.result_storage, candidate.check.result().0);
    assert_eq!(
        candidate.result_assignment.value,
        candidate_result_value(definition, &candidate)
    );
    assert_eq!(
        candidate.operand_loads.map(|operand| operand.value),
        success_operand_loads(definition, &candidate)
    );

    let check = definition.block(candidate.check_block).unwrap();
    let success = definition.block(candidate.success_block).unwrap();
    let join = definition.block(candidate.join_block).unwrap();
    assert_eq!(
        candidate.check_span,
        check.terminator.as_ref().unwrap().span()
    );
    assert_eq!(
        candidate.result_assignment.span,
        success.instructions[2].span()
    );
    assert_eq!(candidate.result_assignment.site.instruction, 2);
    assert_eq!(candidate.result_store.instruction, 3);
    assert_eq!(candidate.result_store_span, success.instructions[3].span());
    assert_eq!(
        candidate.success_edge_span,
        success.terminator.as_ref().unwrap().span()
    );
    assert_eq!(candidate.result_reload.span, join.instructions[0].span());
    assert_eq!(candidate.result_reload.site.instruction, 0);
}

#[test]
fn primitive_folding_can_expose_exact_carrier_sources() {
    let input = lower_source_to_final_mir("fn main() -> i64 { return ((20 + 1) / (2 + 1)); }");
    assert_eq!(
        only_observation(&input),
        CheckedIntegerProtocolObservation::Rejected {
            check_block: checked_block(&input),
            reason: CheckedIntegerProtocolRejectionReason::DynamicOperand,
        }
    );

    let schedule =
        resolve_exact_mir_pass_schedule(&[primitive_constant_folding::IDENTITY]).unwrap();
    let folded = run_mir_pipeline_with_occurrences(input, &schedule)
        .result
        .unwrap();
    assert_eq!(
        only_candidate(folded.program()).constant,
        PrimitiveConstant::I64(7)
    );
}

#[test]
fn dynamic_and_partially_constant_operands_are_ordinary_rejections() {
    for source in [
        "fn divide(value: i64) -> i64 { return value / 2; } fn main() -> i64 { return 0; }",
        "fn divide(value: i64) -> i64 { return 8 / value; } fn main() -> i64 { return 0; }",
        "fn shift(value: u64) -> u64 { return value << 2u; } fn main() -> i64 { return 0; }",
        "fn shift(count: u64) -> u64 { return 8u << count; } fn main() -> i64 { return 0; }",
    ] {
        let program = lower_source_to_final_mir(source);
        assert!(matches!(
            only_observation(&program),
            CheckedIntegerProtocolObservation::Rejected {
                reason: CheckedIntegerProtocolRejectionReason::DynamicOperand,
                ..
            }
        ));
    }
}

#[test]
fn nested_checked_results_do_not_escape_the_narrow_carrier_query() {
    let program = lower_source_to_final_mir("fn main() -> i64 { return (8 / 2) / 2; }");
    let observations = observations(&program);
    assert_eq!(observations.len(), 2);
    assert_eq!(
        observations
            .iter()
            .filter(|observation| matches!(
                observation,
                CheckedIntegerProtocolObservation::Candidate(_)
            ))
            .count(),
        1
    );
    assert_eq!(
        observations
            .iter()
            .filter(|observation| matches!(
                observation,
                CheckedIntegerProtocolObservation::Rejected {
                    reason: CheckedIntegerProtocolRejectionReason::DynamicOperand,
                    ..
                }
            ))
            .count(),
        1
    );
}

#[test]
fn statically_failing_protocols_retain_the_exact_reason() {
    for (source, reason) in [
        (
            "fn main() -> i64 { return 8 / 0; }",
            MirTerminationReason::IntegerDivisionByZero,
        ),
        (
            "fn main() -> i64 { return 8 % 0; }",
            MirTerminationReason::IntegerRemainderByZero,
        ),
        (
            "fn main() -> i64 { return 8 << 64u; }",
            MirTerminationReason::ShiftCountOutOfRange,
        ),
    ] {
        let program = lower_source_to_final_mir(source);
        assert!(matches!(
            only_observation(&program),
            CheckedIntegerProtocolObservation::Rejected {
                reason: CheckedIntegerProtocolRejectionReason::StaticFailure(actual),
                ..
            } if actual == reason
        ));
    }
}

#[test]
fn duplicate_and_nondominating_carrier_writes_are_rejected() {
    let mut duplicate = lower_source_to_final_mir("fn main() -> i64 { return 8 / 2; }");
    let candidate = only_candidate(&duplicate);
    let definition = entry_definition_mut(&mut duplicate);
    let store = definition.body.blocks[candidate.operands[0].store.block.index()].instructions
        [candidate.operands[0].store.instruction]
        .clone();
    definition.body.blocks[candidate.check_block.index()]
        .instructions
        .push(store);
    assert_rejected(
        &duplicate,
        CheckedIntegerProtocolRejectionReason::DynamicOperand,
    );

    let mut nondominating = lower_source_to_final_mir("fn main() -> i64 { return 8 / 2; }");
    let candidate = only_candidate(&nondominating);
    let definition = entry_definition_mut(&mut nondominating);
    let store = definition.body.blocks[candidate.operands[0].store.block.index()]
        .instructions
        .remove(candidate.operands[0].store.instruction);
    let block = BlockId::new(definition.callable(), definition.body.blocks.len());
    definition.body.blocks.push(MirBasicBlock {
        id: block,
        instructions: vec![store],
        terminator: Some(MirTerminator::Return {
            value: None,
            span: definition.span,
        }),
        span: definition.span,
    });
    assert_rejected(
        &nondominating,
        CheckedIntegerProtocolRejectionReason::DynamicOperand,
    );
}

#[test]
fn mismatched_success_failure_and_join_shapes_are_noncanonical() {
    let base = lower_source_to_final_mir("fn main() -> i64 { return 8 / 2; }");
    let candidate = only_candidate(&base);

    let mut wrong_operation = base.clone();
    let definition = entry_definition_mut(&mut wrong_operation);
    let MirInstruction::Assign(assignment) =
        &mut definition.body.blocks[candidate.success_block.index()].instructions[2]
    else {
        unreachable!();
    };
    let MirRvalueKind::IntegerDivision { operation, .. } = &mut assignment.rvalue.kind else {
        unreachable!();
    };
    operation.kind = crate::mir::MirIntegerDivisionKind::Remainder;
    assert_rejected(
        &wrong_operation,
        CheckedIntegerProtocolRejectionReason::NonCanonicalTopology,
    );

    let mut wrong_failure = base.clone();
    entry_definition_mut(&mut wrong_failure).body.blocks[candidate.failure_block.index()]
        .terminator = Some(MirTerminator::Terminate {
        reason: MirTerminationReason::OptionalAccessFailure,
        span: candidate.check_span,
    });
    assert_rejected(
        &wrong_failure,
        CheckedIntegerProtocolRejectionReason::NonCanonicalTopology,
    );

    let mut shared_join = base;
    let definition = entry_definition_mut(&mut shared_join);
    definition.body.blocks[candidate.failure_block.index()].terminator =
        Some(MirTerminator::Goto {
            target: candidate.join_block,
            span: candidate.check_span,
        });
    assert_rejected(
        &shared_join,
        CheckedIntegerProtocolRejectionReason::NonCanonicalTopology,
    );
}

#[test]
fn metadata_protected_protocol_blocks_are_rejected() {
    let mut program = lower_source_to_final_mir("fn main() -> i64 { return 8 / 2; }");
    let candidate = only_candidate(&program);
    let definition = entry_definition_mut(&mut program);
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

    assert_rejected(
        &program,
        CheckedIntegerProtocolRejectionReason::ProtectedTopology,
    );
}

#[test]
fn observations_follow_callable_then_block_order_deterministically() {
    let program = lower_source_to_final_mir(concat!(
        "fn first() -> i64 { return (8 / 2) + (7 % 3); }\n",
        "fn second() -> i64 { return (1 << 2u) + (8 >> 1u); }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    let first = observations(&program);
    let second = observations(&program);
    assert_eq!(first, second);
    assert_eq!(first.len(), 4);
    assert!(first.windows(2).all(|window| {
        let left = observation_block(&window[0]);
        let right = observation_block(&window[1]);
        left.callable() < right.callable()
            || (left.callable() == right.callable() && left.index() < right.index())
    }));
}

#[test]
fn malformed_block_and_storage_identities_are_query_errors() {
    let mut bad_block = lower_source_to_final_mir("fn main() -> i64 { return 8 / 2; }");
    let owner = bad_block.entry_function;
    let definition = entry_definition_mut(&mut bad_block);
    let MirTerminator::IntegerDivisorCheck { success_target, .. } = definition
        .body
        .blocks
        .iter_mut()
        .find_map(|block| block.terminator.as_mut())
        .expect("entry block has a terminator")
    else {
        unreachable!();
    };
    *success_target = BlockId::new(FunctionId::new(owner.index() + 1), 0);
    assert!(matches!(
        observe_checked_integer_protocols((&*definition).into()),
        Err(MirRewriteError::InvalidReference {
            failure: MirReferenceFailure::Foreign,
            ..
        })
    ));

    let mut bad_storage = lower_source_to_final_mir("fn main() -> i64 { return 8 / 2; }");
    let definition = entry_definition_mut(&mut bad_storage);
    let owner = definition.callable();
    let checked = definition
        .body
        .blocks
        .iter_mut()
        .find_map(|block| match block.terminator.as_mut() {
            Some(MirTerminator::IntegerDivisorCheck { check, .. }) => Some(check),
            _ => None,
        })
        .unwrap();
    checked.dividend = StorageId::new(owner, definition.storage.len() + 10);
    assert!(matches!(
        observe_checked_integer_protocols((&*definition).into()),
        Err(MirRewriteError::InvalidReference {
            failure: MirReferenceFailure::Unknown,
            ..
        })
    ));
}

fn checked_block(program: &crate::mir::MirProgram) -> BlockId {
    let definition = program.definitions.get(program.entry_function).unwrap();
    definition
        .body
        .blocks
        .iter()
        .find(|block| {
            matches!(
                block.terminator,
                Some(MirTerminator::IntegerDivisorCheck { .. })
                    | Some(MirTerminator::ShiftCountCheck { .. })
            )
        })
        .unwrap()
        .id
}

fn assert_rejected(
    program: &crate::mir::MirProgram,
    expected: CheckedIntegerProtocolRejectionReason,
) {
    assert!(matches!(
        only_observation(program),
        CheckedIntegerProtocolObservation::Rejected { reason, .. } if reason == expected
    ));
}

fn observation_block(observation: &CheckedIntegerProtocolObservation) -> BlockId {
    match observation {
        CheckedIntegerProtocolObservation::Candidate(candidate) => candidate.check_block,
        CheckedIntegerProtocolObservation::Rejected { check_block, .. } => *check_block,
    }
}

fn candidate_result_value(
    definition: &crate::mir::MirFunctionDefinition,
    candidate: &CheckedIntegerProtocolCandidate,
) -> ValueId {
    let MirInstruction::Assign(assignment) =
        &definition.body.blocks[candidate.success_block.index()].instructions[2]
    else {
        unreachable!();
    };
    assignment.result
}

fn success_operand_loads(
    definition: &crate::mir::MirFunctionDefinition,
    candidate: &CheckedIntegerProtocolCandidate,
) -> [ValueId; 2] {
    let block = &definition.body.blocks[candidate.success_block.index()];
    std::array::from_fn(|index| match &block.instructions[index] {
        MirInstruction::Assign(assignment) => assignment.result,
        _ => unreachable!(),
    })
}
