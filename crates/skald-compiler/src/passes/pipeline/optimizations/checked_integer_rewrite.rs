//! Atomic rewriting of one revalidated checked-integer protocol.

use std::collections::{HashMap, HashSet};

use crate::mir::{
    rewrite::{MirCallableEdit, MirLocalCfgFacts, MirRewriteError},
    BlockId, MirInstruction, MirPlace, MirRvalueKind, MirTerminator, StorageId, ValueId,
};

use super::{
    checked_integer_protocol::{
        CheckedIntegerConstantCarrier, CheckedIntegerInstructionSite,
        CheckedIntegerProtocolCandidate, CheckedIntegerProtocolCheck, CheckedIntegerValueSite,
    },
    primitive_evaluation::{evaluate_rvalue, PrimitiveEvaluation},
};

/// Structural result owned by one successful protocol transaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct CheckedIntegerProtocolRewrite {
    pub(super) removed_operand_loads: usize,
}

/// Revalidates and rewrites one candidate without publishing intermediate MIR.
pub(super) fn rewrite_checked_integer_protocol(
    edit: &mut MirCallableEdit,
    candidate: &CheckedIntegerProtocolCandidate,
) -> Result<CheckedIntegerProtocolRewrite, MirRewriteError> {
    revalidate(edit, candidate)?;

    edit.rewrite_block_terminator(candidate.check_block, |_| {
        Some(MirTerminator::Goto {
            target: candidate.success_block,
            span: candidate.check_span,
        })
    })?;
    edit.rewrite_block_instructions(candidate.success_block, |instructions| {
        let mut result = instructions[candidate.result_assignment.site.instruction].clone();
        let MirInstruction::Assign(assignment) = &mut result else {
            unreachable!("candidate revalidation accepted a checked result assignment");
        };
        assignment.rvalue.kind = candidate.constant.into_rvalue_kind();
        vec![
            result,
            instructions[candidate.result_store.instruction].clone(),
        ]
    })?;
    for load in candidate.operand_loads {
        edit.remove_value(load.value)?;
    }

    Ok(CheckedIntegerProtocolRewrite {
        removed_operand_loads: candidate.operand_loads.len(),
    })
}

fn revalidate(
    edit: &MirCallableEdit,
    candidate: &CheckedIntegerProtocolCandidate,
) -> Result<(), MirRewriteError> {
    validate_live_identities(edit, candidate)?;
    let cfg = edit.local_cfg_facts()?;
    let predecessors = predecessors(&cfg);
    let protected = cfg
        .protected_roots()
        .iter()
        .map(|root| root.block())
        .collect::<HashSet<_>>();

    if !check_matches(edit, candidate)
        || !success_matches(edit, candidate)
        || !failure_matches(edit, candidate)
        || !reload_matches(edit, candidate)
        || !has_only_predecessor(
            &predecessors,
            candidate.success_block,
            candidate.check_block,
        )
        || !has_only_predecessor(
            &predecessors,
            candidate.failure_block,
            candidate.check_block,
        )
        || !has_only_predecessor(&predecessors, candidate.join_block, candidate.success_block)
        || candidate
            .operands
            .iter()
            .any(|carrier| !constant_carrier_matches(edit, &cfg, carrier, candidate.check_block))
        || storage_write_sites(edit, candidate.result_storage).as_slice()
            != [candidate.result_store]
        || [
            candidate.check_block,
            candidate.success_block,
            candidate.failure_block,
            candidate.join_block,
        ]
        .iter()
        .any(|block| protected.contains(block))
    {
        return Err(stale(edit));
    }
    Ok(())
}

fn validate_live_identities(
    edit: &MirCallableEdit,
    candidate: &CheckedIntegerProtocolCandidate,
) -> Result<(), MirRewriteError> {
    for block in [
        candidate.check_block,
        candidate.success_block,
        candidate.failure_block,
        candidate.join_block,
        candidate.operands[0].source_assignment.block,
        candidate.operands[0].store.block,
        candidate.operands[1].source_assignment.block,
        candidate.operands[1].store.block,
    ] {
        edit.block(block)?;
    }
    for storage in [
        candidate.operands[0].storage,
        candidate.operands[1].storage,
        candidate.result_storage,
    ] {
        edit.storage(storage)?;
    }
    for value in [
        candidate.operands[0].source_value,
        candidate.operands[1].source_value,
        candidate.operand_loads[0].value,
        candidate.operand_loads[1].value,
        candidate.result_assignment.value,
        candidate.result_reload.value,
    ] {
        edit.value(value)?;
    }
    Ok(())
}

fn check_matches(edit: &MirCallableEdit, candidate: &CheckedIntegerProtocolCandidate) -> bool {
    let Ok(block) = edit.block(candidate.check_block) else {
        return false;
    };
    match (candidate.check, block.terminator.as_ref()) {
        (
            CheckedIntegerProtocolCheck::Division(expected),
            Some(MirTerminator::IntegerDivisorCheck {
                check,
                success_target,
                failure_target,
                span,
            }),
        ) => {
            *check == expected
                && *success_target == candidate.success_block
                && *failure_target == candidate.failure_block
                && *span == candidate.check_span
        }
        (
            CheckedIntegerProtocolCheck::Shift(expected),
            Some(MirTerminator::ShiftCountCheck {
                check,
                success_target,
                failure_target,
                span,
            }),
        ) => {
            *check == expected
                && *success_target == candidate.success_block
                && *failure_target == candidate.failure_block
                && *span == candidate.check_span
        }
        _ => false,
    }
}

fn success_matches(edit: &MirCallableEdit, candidate: &CheckedIntegerProtocolCandidate) -> bool {
    let Ok(block) = edit.block(candidate.success_block) else {
        return false;
    };
    let [MirInstruction::Assign(first), MirInstruction::Assign(second), MirInstruction::Assign(result), MirInstruction::Store(store)] =
        block.instructions.as_slice()
    else {
        return false;
    };
    let [(first_storage, first_type), (second_storage, second_type)] = candidate.check.operands();
    let (_, result_type) = candidate.check.result();
    value_site_matches(
        candidate.operand_loads[0],
        first.result,
        first.span,
        candidate.success_block,
        0,
    ) && value_site_matches(
        candidate.operand_loads[1],
        second.result,
        second.span,
        candidate.success_block,
        1,
    ) && is_exact_load(&first.rvalue.kind, first_storage)
        && first.rvalue.ty == first_type
        && is_exact_load(&second.rvalue.kind, second_storage)
        && second.rvalue.ty == second_type
        && value_site_matches(
            candidate.result_assignment,
            result.result,
            result.span,
            candidate.success_block,
            2,
        )
        && checked_operation_matches(
            candidate.check,
            &result.rvalue.kind,
            first.result,
            second.result,
        )
        && result.rvalue.ty == result_type
        && candidate.result_store
            == (CheckedIntegerInstructionSite {
                block: candidate.success_block,
                instruction: 3,
            })
        && store.destination == MirPlace::base(candidate.result_storage)
        && store.value == candidate.result_assignment.value
        && store.authorization.is_none()
        && store.final_authorization.is_none()
        && store.span == candidate.result_store_span
        && matches!(
            block.terminator,
            Some(MirTerminator::Goto { target, span })
                if target == candidate.join_block && span == candidate.success_edge_span
        )
}

fn failure_matches(edit: &MirCallableEdit, candidate: &CheckedIntegerProtocolCandidate) -> bool {
    edit.block(candidate.failure_block).is_ok_and(|block| {
        block.instructions.is_empty()
            && matches!(
                block.terminator,
                Some(MirTerminator::Terminate { reason, .. })
                    if reason == candidate.check.failure_reason()
            )
    })
}

fn reload_matches(edit: &MirCallableEdit, candidate: &CheckedIntegerProtocolCandidate) -> bool {
    let Ok(block) = edit.block(candidate.join_block) else {
        return false;
    };
    let Some(MirInstruction::Assign(load)) = block.instructions.first() else {
        return false;
    };
    let (_, result_type) = candidate.check.result();
    value_site_matches(
        candidate.result_reload,
        load.result,
        load.span,
        candidate.join_block,
        0,
    ) && is_exact_load(&load.rvalue.kind, candidate.result_storage)
        && load.rvalue.ty == result_type
}

fn checked_operation_matches(
    check: CheckedIntegerProtocolCheck,
    kind: &MirRvalueKind,
    first: ValueId,
    second: ValueId,
) -> bool {
    match (check, kind) {
        (
            CheckedIntegerProtocolCheck::Division(check),
            MirRvalueKind::IntegerDivision {
                operation,
                dividend,
                divisor,
            },
        ) => *operation == check.operation && *dividend == first && *divisor == second,
        (
            CheckedIntegerProtocolCheck::Shift(check),
            MirRvalueKind::Shift {
                operation,
                left,
                count,
            },
        ) => *operation == check.operation && *left == first && *count == second,
        _ => false,
    }
}

fn value_site_matches(
    expected: CheckedIntegerValueSite,
    value: ValueId,
    span: crate::source::Span,
    block: BlockId,
    instruction: usize,
) -> bool {
    expected.value == value
        && expected.span == span
        && expected.site == CheckedIntegerInstructionSite { block, instruction }
}

fn constant_carrier_matches(
    edit: &MirCallableEdit,
    cfg: &MirLocalCfgFacts,
    carrier: &CheckedIntegerConstantCarrier,
    check_block: BlockId,
) -> bool {
    if !dominates(cfg, carrier.store.block, check_block)
        || !dominates(cfg, carrier.source_assignment.block, carrier.store.block)
        || (carrier.source_assignment.block == carrier.store.block
            && carrier.source_assignment.instruction >= carrier.store.instruction)
        || storage_write_sites(edit, carrier.storage).as_slice() != [carrier.store]
    {
        return false;
    }
    let Ok(source_block) = edit.block(carrier.source_assignment.block) else {
        return false;
    };
    let Some(MirInstruction::Assign(assignment)) = source_block
        .instructions
        .get(carrier.source_assignment.instruction)
    else {
        return false;
    };
    let Ok(store_block) = edit.block(carrier.store.block) else {
        return false;
    };
    let Some(MirInstruction::Store(store)) =
        store_block.instructions.get(carrier.store.instruction)
    else {
        return false;
    };
    let PrimitiveEvaluation::Constant(constant) =
        evaluate_rvalue(&assignment.rvalue.kind, |_| None)
    else {
        return false;
    };
    let expected_type = carrier.constant.ty();
    assignment.result == carrier.source_value
        && assignment.span == carrier.source_span
        && assignment.rvalue.ty == expected_type
        && constant == carrier.constant
        && edit
            .value(carrier.source_value)
            .is_ok_and(|value| value.ty == expected_type)
        && store.destination == MirPlace::base(carrier.storage)
        && store.value == carrier.source_value
        && store.authorization.is_none()
        && store.final_authorization.is_none()
        && store.span == carrier.store_span
}

fn storage_write_sites(
    edit: &MirCallableEdit,
    storage: StorageId,
) -> Vec<CheckedIntegerInstructionSite> {
    edit.block_order()
        .iter()
        .flat_map(|block| {
            edit.block(*block)
                .expect("block order contains only live blocks")
                .instructions
                .iter()
                .enumerate()
                .filter_map(move |(instruction, value)| {
                    matches!(
                        value,
                        MirInstruction::Store(store)
                            if store.destination == MirPlace::base(storage)
                    )
                    .then_some(CheckedIntegerInstructionSite {
                        block: *block,
                        instruction,
                    })
                })
        })
        .collect()
}

fn predecessors(cfg: &MirLocalCfgFacts) -> HashMap<BlockId, HashSet<BlockId>> {
    let mut predecessors = HashMap::<_, HashSet<_>>::new();
    for block in cfg.blocks() {
        for successor in block.successors() {
            predecessors
                .entry(*successor)
                .or_default()
                .insert(block.block());
        }
    }
    predecessors
}

fn has_only_predecessor(
    predecessors: &HashMap<BlockId, HashSet<BlockId>>,
    block: BlockId,
    expected: BlockId,
) -> bool {
    predecessors.get(&block) == Some(&HashSet::from([expected]))
}

fn dominates(cfg: &MirLocalCfgFacts, dominator: BlockId, target: BlockId) -> bool {
    dominator == target
        || (reachable(cfg, target, None) && !reachable(cfg, target, Some(dominator)))
}

fn reachable(cfg: &MirLocalCfgFacts, target: BlockId, excluded: Option<BlockId>) -> bool {
    if excluded == Some(cfg.entry()) {
        return false;
    }
    let mut pending = vec![cfg.entry()];
    let mut visited = HashSet::new();
    while let Some(block) = pending.pop() {
        if block == target {
            return true;
        }
        if !visited.insert(block) {
            continue;
        }
        if let Some(facts) = cfg.block(block) {
            pending.extend(
                facts
                    .successors()
                    .iter()
                    .copied()
                    .filter(|successor| Some(*successor) != excluded),
            );
        }
    }
    false
}

fn is_exact_load(kind: &MirRvalueKind, storage: StorageId) -> bool {
    matches!(kind, MirRvalueKind::Load(place) if *place == MirPlace::base(storage))
}

fn stale(edit: &MirCallableEdit) -> MirRewriteError {
    MirRewriteError::StaleCallableSnapshot {
        callable: edit.callable(),
        subject: "checked-integer protocol",
    }
}

#[cfg(test)]
#[path = "checked_integer_rewrite/tests.rs"]
mod tests;
