use crate::{
    identity::FunctionId,
    mir::{
        rewrite::{rewrite_program, MirLocalIdentity, MirRewriteError},
        BlockId, MirDefinitionRef, MirInstruction, MirPlace, MirRvalueKind, MirTerminator,
    },
    passes::verify_final_mir,
    test_support::lower_source_to_final_mir,
};

use super::*;
use crate::passes::pipeline::optimizations::checked_integer_folding::{
    CheckedIntegerFoldPlan, CheckedIntegerFoldSelection,
};

fn only_candidate(program: &crate::mir::MirProgram) -> CheckedIntegerProtocolCandidate {
    let plan = CheckedIntegerFoldPlan::prepare(program, CheckedIntegerFoldSelection::All).unwrap();
    let candidates = plan.candidates().cloned().collect::<Vec<_>>();
    assert_eq!(candidates.len(), 1, "{candidates:#?}");
    candidates.into_iter().next().unwrap()
}

fn definition(
    program: &crate::mir::MirProgram,
    callable: crate::identity::CallableId,
) -> MirDefinitionRef<'_> {
    program
        .executable_definitions()
        .find(|definition| definition.callable() == callable)
        .unwrap()
}

fn rewrite_only_candidate(
    program: crate::mir::MirProgram,
    candidate: &CheckedIntegerProtocolCandidate,
) -> crate::mir::rewrite::MirProgramRewriteResult {
    rewrite_program(program, |callable, edit| {
        if callable == candidate.check_block.callable() {
            assert_eq!(
                rewrite_checked_integer_protocol(edit, candidate)?,
                CheckedIntegerProtocolRewrite {
                    removed_operand_loads: 2,
                }
            );
        }
        Ok(())
    })
    .unwrap()
}

#[test]
fn shared_transaction_rewrites_division_and_shift_protocols() {
    for (source, expected) in [
        (
            "fn main() -> i64 { return 17 / 5; }",
            MirRvalueKind::ConstantI64(3),
        ),
        (
            "fn main() -> i64 { return 8 >> 2u; }",
            MirRvalueKind::ConstantI64(2),
        ),
    ] {
        let input = lower_source_to_final_mir(source);
        let candidate = only_candidate(&input);
        let original = definition(&input, candidate.check_block.callable());
        let original_storage = [
            candidate.operands[0].storage,
            candidate.operands[1].storage,
            candidate.result_storage,
        ]
        .map(|storage| original.storage(storage).unwrap().clone());
        let original_paths = original.body().path_conditions.clone();
        let original_logical = original.body().logical_expressions.clone();
        let result = rewrite_only_candidate(input, &candidate);
        let report = result
            .callables
            .iter()
            .find(|report| report.callable == candidate.check_block.callable())
            .unwrap();

        assert_eq!(report.changes.values.removed, 2);
        assert_eq!(report.changes.storage.removed, 0);
        assert_eq!(report.changes.blocks.removed, 0);
        for operand in candidate.operand_loads {
            assert!(matches!(
                report.maps.values.committed(operand.value),
                Err(MirRewriteError::DeletedIdentity {
                    identity: MirLocalIdentity::Value(value),
                }) if value == operand.value
            ));
        }

        let check = report.maps.blocks.committed(candidate.check_block).unwrap();
        let success = report
            .maps
            .blocks
            .committed(candidate.success_block)
            .unwrap();
        let join = report.maps.blocks.committed(candidate.join_block).unwrap();
        let failure = report
            .maps
            .blocks
            .committed(candidate.failure_block)
            .unwrap();
        let result_value = report
            .maps
            .values
            .committed(candidate.result_assignment.value)
            .unwrap();
        let reload_value = report
            .maps
            .values
            .committed(candidate.result_reload.value)
            .unwrap();
        let rewritten = definition(&result.program, candidate.check_block.callable());

        assert_eq!(
            [
                candidate.operands[0].storage,
                candidate.operands[1].storage,
                candidate.result_storage,
            ]
            .map(|storage| rewritten.storage(storage).unwrap().clone()),
            original_storage
        );
        assert_eq!(rewritten.body().path_conditions, original_paths);
        assert_eq!(rewritten.body().logical_expressions, original_logical);
        assert!(matches!(
            rewritten.block(check).unwrap().terminator,
            Some(MirTerminator::Goto { target, span })
                if target == success && span == candidate.check_span
        ));
        let success_block = rewritten.block(success).unwrap();
        let [MirInstruction::Assign(assignment), MirInstruction::Store(store)] =
            success_block.instructions.as_slice()
        else {
            panic!("rewritten success block must contain constant assignment and result store");
        };
        assert_eq!(assignment.result, result_value);
        assert_eq!(assignment.rvalue.kind, expected);
        assert_eq!(assignment.rvalue.ty, candidate.check.result().1);
        assert_eq!(assignment.span, candidate.result_assignment.span);
        assert_eq!(store.value, result_value);
        assert_eq!(store.destination, MirPlace::base(candidate.result_storage));
        assert_eq!(store.span, candidate.result_store_span);
        assert!(matches!(
            success_block.terminator,
            Some(MirTerminator::Goto { target, span })
                if target == join && span == candidate.success_edge_span
        ));
        assert!(matches!(
            rewritten.block(failure).unwrap().terminator,
            Some(MirTerminator::Terminate { reason, .. })
                if reason == candidate.check.failure_reason()
        ));
        let MirInstruction::Assign(reload) = &rewritten.block(join).unwrap().instructions[0] else {
            panic!("join must retain the result reload");
        };
        assert_eq!(reload.result, reload_value);
        assert!(is_exact_load(&reload.rvalue.kind, candidate.result_storage));
        verify_final_mir(result.program).unwrap();
    }
}

#[test]
fn successful_rewrite_has_deterministic_dense_commit_maps() {
    let input = lower_source_to_final_mir("fn main() -> i64 { return 17 % 5; }");
    let candidate = only_candidate(&input);
    let first = rewrite_only_candidate(input.clone(), &candidate);
    let second = rewrite_only_candidate(input, &candidate);

    assert_eq!(first, second);
    let rewritten = definition(&first.program, candidate.check_block.callable());
    assert!(rewritten
        .values()
        .iter()
        .enumerate()
        .all(|(index, value)| value.id.index() == index));
    assert!(rewritten
        .body()
        .blocks
        .iter()
        .enumerate()
        .all(|(index, block)| block.id.index() == index));
}

#[test]
fn unrelated_proof_and_static_lifecycle_metadata_survive_the_transaction() {
    let input = lower_source_to_final_mir(concat!(
        "class State { static value: i64 = 1; init() {} }\n",
        "fn main() -> i64 {\n",
        "  var value: i64 = 0;\n",
        "  if (true && true) { value = State.value; }\n",
        "  return value + (8 / 2);\n",
        "}\n",
    ));
    let candidate = only_candidate(&input);
    let original = definition(&input, candidate.check_block.callable());
    assert!(!original.body().path_conditions.is_empty());
    assert!(!original.body().logical_expressions.is_empty());
    assert!(input.static_lifecycle.is_some());
    let paths = original.body().path_conditions.clone();
    let logical = original.body().logical_expressions.clone();
    let lifecycle = input.static_lifecycle.clone();

    let result = rewrite_only_candidate(input, &candidate);
    let rewritten = definition(&result.program, candidate.check_block.callable());
    assert_eq!(rewritten.body().path_conditions, paths);
    assert_eq!(rewritten.body().logical_expressions, logical);
    assert_eq!(result.program.static_lifecycle, lifecycle);
    verify_final_mir(result.program).unwrap();
}

#[test]
fn stale_revalidation_never_partially_applies_the_protocol_rewrite() {
    let input = lower_source_to_final_mir("fn main() -> i64 { return 8 / 2; }");
    let candidate = only_candidate(&input);
    let error = rewrite_program(input, |callable, edit| {
        if callable != candidate.check_block.callable() {
            return Ok(());
        }
        edit.rewrite_block_instructions(candidate.success_block, |instructions| {
            let mut changed = instructions.to_vec();
            changed.swap(0, 1);
            changed
        })?;
        let before_attempt = edit.clone();
        let error = rewrite_checked_integer_protocol(edit, &candidate).unwrap_err();
        assert_eq!(
            edit, &before_attempt,
            "failed revalidation must not mutate MIR"
        );
        Err(error)
    })
    .unwrap_err();

    assert_eq!(
        error,
        MirRewriteError::StaleCallableSnapshot {
            callable: candidate.check_block.callable(),
            subject: "checked-integer protocol",
        }
    );
}

#[test]
fn foreign_and_deleted_candidate_identities_fail_before_mutation() {
    let input = lower_source_to_final_mir("fn main() -> i64 { return 8 / 2; }");
    let candidate = only_candidate(&input);
    let mut foreign = candidate.clone();
    foreign.check_block = BlockId::new(FunctionId::new(input.entry_function.index() + 1), 0);
    let foreign_error = rewrite_program(input.clone(), |callable, edit| {
        if callable == candidate.check_block.callable() {
            rewrite_checked_integer_protocol(edit, &foreign)?;
        }
        Ok(())
    })
    .unwrap_err();
    assert!(matches!(
        foreign_error,
        MirRewriteError::ForeignIdentity {
            identity: MirLocalIdentity::Block(block),
            ..
        } if block == foreign.check_block
    ));

    let deleted_error = rewrite_program(input, |callable, edit| {
        if callable != candidate.check_block.callable() {
            return Ok(());
        }
        edit.remove_value(candidate.operand_loads[0].value)?;
        let before_attempt = edit.clone();
        let error = rewrite_checked_integer_protocol(edit, &candidate).unwrap_err();
        assert_eq!(edit, &before_attempt);
        Err(error)
    })
    .unwrap_err();
    assert!(matches!(
        deleted_error,
        MirRewriteError::DeletedIdentity {
            identity: MirLocalIdentity::Value(value),
        } if value == candidate.operand_loads[0].value
    ));
}

#[test]
fn central_verifier_accepts_only_the_complete_protocol_transaction() {
    let input = lower_source_to_final_mir("fn main() -> i64 { return 8 / 2; }");
    let candidate = only_candidate(&input);

    let terminator_only = rewrite_program(input.clone(), |callable, edit| {
        if callable == candidate.check_block.callable() {
            edit.rewrite_block_terminator(candidate.check_block, |_| {
                Some(MirTerminator::Goto {
                    target: candidate.success_block,
                    span: candidate.check_span,
                })
            })?;
        }
        Ok(())
    })
    .unwrap();
    assert!(verify_final_mir(terminator_only.program).is_err());

    let assignment_only = rewrite_program(input.clone(), |callable, edit| {
        if callable == candidate.check_block.callable() {
            edit.rewrite_block_instructions(candidate.success_block, |instructions| {
                let mut instructions = instructions.to_vec();
                let MirInstruction::Assign(assignment) =
                    &mut instructions[candidate.result_assignment.site.instruction]
                else {
                    unreachable!();
                };
                assignment.rvalue.kind = candidate.constant.into_rvalue_kind();
                instructions
            })?;
        }
        Ok(())
    })
    .unwrap();
    assert!(verify_final_mir(assignment_only.program).is_err());

    let complete = rewrite_only_candidate(input, &candidate);
    verify_final_mir(complete.program).unwrap();
}
