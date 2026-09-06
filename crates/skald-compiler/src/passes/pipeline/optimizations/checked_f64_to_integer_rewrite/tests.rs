use crate::{
    identity::FunctionId,
    mir::{
        rewrite::{rewrite_program, MirLocalIdentity, MirRewriteError},
        test_fixtures::checked_primitive_cast_program,
        BlockId, MirDefinitionRef, MirInstruction, MirIntegerType, MirPlace, MirRvalueKind,
        MirTerminator,
    },
    passes::verify_final_mir,
};

use super::*;
use crate::passes::pipeline::optimizations::checked_f64_to_integer_folding::CheckedF64ToIntegerFoldPlan;

fn only_candidate(program: &crate::mir::MirProgram) -> CheckedF64ToIntegerProtocolCandidate {
    let plan = CheckedF64ToIntegerFoldPlan::prepare(program).unwrap();
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
    candidate: &CheckedF64ToIntegerProtocolCandidate,
) -> crate::mir::rewrite::MirProgramRewriteResult {
    rewrite_program(program, |callable, edit| {
        if callable == candidate.check_block.callable() {
            assert_eq!(
                rewrite_checked_f64_to_integer_protocol(edit, candidate)?,
                CheckedF64ToIntegerProtocolRewrite {
                    removed_protocol_values: 1,
                }
            );
        }
        Ok(())
    })
    .unwrap()
}

#[test]
fn complete_transaction_rewrites_every_target_and_preserves_protocol_storage() {
    for (target, bits, expected_bits, expected) in [
        (
            MirIntegerType::I64,
            0xc022_6666_6666_6666,
            (-9i64) as u64,
            MirRvalueKind::ConstantI64(-9),
        ),
        (
            MirIntegerType::U64,
            0x43ef_ffff_ffff_ffff,
            18_446_744_073_709_549_568,
            MirRvalueKind::ConstantU64(18_446_744_073_709_549_568),
        ),
        (
            MirIntegerType::U8,
            0x406f_ffff_ffff_ffff,
            255,
            MirRvalueKind::ConstantU8(255),
        ),
    ] {
        let input = checked_primitive_cast_program(bits, target, expected_bits);
        let candidate = only_candidate(&input);
        let original = definition(&input, candidate.check_block.callable());
        let original_storage = [candidate.source.storage, candidate.result_storage]
            .map(|storage| original.storage(storage).unwrap().clone());
        let result = rewrite_only_candidate(input, &candidate);
        let report = result
            .callables
            .iter()
            .find(|report| report.callable == candidate.check_block.callable())
            .unwrap();

        assert_eq!(report.changes.values.removed, 1);
        assert_eq!(report.changes.storage.removed, 0);
        assert_eq!(report.changes.blocks.removed, 0);
        assert!(matches!(
            report.maps.values.committed(candidate.source_load.value),
            Err(MirRewriteError::DeletedIdentity {
                identity: MirLocalIdentity::Value(value),
            }) if value == candidate.source_load.value
        ));

        let check = report.maps.blocks.committed(candidate.check_block).unwrap();
        let success = report
            .maps
            .blocks
            .committed(candidate.success_block)
            .unwrap();
        let join = report.maps.blocks.committed(candidate.join_block).unwrap();
        let result_value = report
            .maps
            .values
            .committed(candidate.result_assignment.value)
            .unwrap();
        let rewritten = definition(&result.program, candidate.check_block.callable());
        assert_eq!(
            [candidate.source.storage, candidate.result_storage]
                .map(|storage| rewritten.storage(storage).unwrap().clone()),
            original_storage
        );
        assert!(matches!(
            rewritten.block(check).unwrap().terminator,
            Some(MirTerminator::Goto { target, span })
                if target == success && span == candidate.check_span
        ));
        let [MirInstruction::Assign(assignment), MirInstruction::Store(store)] =
            rewritten.block(success).unwrap().instructions.as_slice()
        else {
            panic!("rewritten success must contain the constant and result store");
        };
        assert_eq!(assignment.result, result_value);
        assert_eq!(assignment.rvalue.kind, expected);
        assert_eq!(assignment.span, candidate.result_assignment.span);
        assert_eq!(store.destination, MirPlace::base(candidate.result_storage));
        assert_eq!(store.value, result_value);
        assert_eq!(store.span, candidate.result_store_span);
        assert!(matches!(
            rewritten.block(join).unwrap().instructions.first(),
            Some(MirInstruction::Assign(load))
                if is_exact_load(&load.rvalue.kind, candidate.result_storage)
        ));
        verify_final_mir(result.program).unwrap();
    }
}

#[test]
fn successful_rewrite_has_deterministic_dense_commit_maps() {
    let input = checked_primitive_cast_program(0x400e_0000_0000_0000, MirIntegerType::I64, 3);
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
fn stale_revalidation_never_partially_applies_the_protocol_rewrite() {
    let input = checked_primitive_cast_program(0x400e_0000_0000_0000, MirIntegerType::I64, 3);
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
        let error = rewrite_checked_f64_to_integer_protocol(edit, &candidate).unwrap_err();
        assert_eq!(edit, &before_attempt);
        Err(error)
    })
    .unwrap_err();

    assert_eq!(
        error,
        MirRewriteError::StaleCallableSnapshot {
            callable: candidate.check_block.callable(),
            subject: "checked floating-to-integer protocol",
        }
    );
}

#[test]
fn foreign_and_deleted_candidate_identities_fail_before_mutation() {
    let input = checked_primitive_cast_program(0x400e_0000_0000_0000, MirIntegerType::I64, 3);
    let candidate = only_candidate(&input);
    let mut foreign = candidate.clone();
    foreign.check_block = BlockId::new(FunctionId::new(input.entry_function.index() + 1), 0);
    let foreign_error = rewrite_program(input.clone(), |callable, edit| {
        if callable == candidate.check_block.callable() {
            rewrite_checked_f64_to_integer_protocol(edit, &foreign)?;
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
        edit.remove_value(candidate.source_load.value)?;
        let before_attempt = edit.clone();
        let error = rewrite_checked_f64_to_integer_protocol(edit, &candidate).unwrap_err();
        assert_eq!(edit, &before_attempt);
        Err(error)
    })
    .unwrap_err();
    assert!(matches!(
        deleted_error,
        MirRewriteError::DeletedIdentity {
            identity: MirLocalIdentity::Value(value),
        } if value == candidate.source_load.value
    ));
}

#[test]
fn central_verifier_accepts_only_the_complete_protocol_transaction() {
    let input = checked_primitive_cast_program(0x400e_0000_0000_0000, MirIntegerType::I64, 3);
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
