//! Atomic rewriting of one revalidated checked-integer protocol.

use std::collections::{HashMap, HashSet};

use crate::mir::{
    rewrite::{MirCallableEdit, MirLocalCfgFacts, MirRewriteError},
    BlockId, MirInstruction, MirPlace, MirRvalueKind, MirTerminator, StorageId, ValueId,
};

use super::{
    checked_integer_evaluation::{
        evaluate_integer_division, evaluate_shift, CheckedIntegerEvaluation,
    },
    checked_integer_topology::{
        CheckedIntegerInstructionSite, CheckedIntegerProtocolCheck,
        CheckedIntegerProtocolOperation, CheckedIntegerProtocolTopology, CheckedIntegerValueSite,
    },
    local_constant::{
        CheckedCarrierPlanEvidence, CheckedCarrierPlanRole, LocalConstantFact,
        LocalConstantProvenanceCategory,
    },
    primitive_evaluation::PrimitiveConstant,
};

/// Solved value and certified storage evidence for one checked operand.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CheckedIntegerOperandEvidence {
    pub(super) storage: StorageId,
    pub(super) source_value: ValueId,
    pub(super) constant: PrimitiveConstant,
    pub(super) propagated: bool,
}

/// Exact immutable source-snapshot input for one checked protocol rewrite.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CheckedIntegerProtocolCandidate {
    pub(super) check: CheckedIntegerProtocolCheck,
    pub(super) check_block: BlockId,
    pub(super) check_span: crate::source::Span,
    pub(super) success_block: BlockId,
    pub(super) failure_block: BlockId,
    pub(super) join_block: BlockId,
    pub(super) operands: [CheckedIntegerOperandEvidence; 2],
    pub(super) operand_loads: [CheckedIntegerValueSite; 2],
    pub(super) result_storage: StorageId,
    pub(super) result_assignment: CheckedIntegerValueSite,
    pub(super) result_store: CheckedIntegerInstructionSite,
    pub(super) result_store_span: crate::source::Span,
    pub(super) success_edge_span: crate::source::Span,
    pub(super) result_reload: CheckedIntegerValueSite,
    pub(super) constant: PrimitiveConstant,
    carriers: [CheckedCarrierPlanEvidence; 3],
}

impl CheckedIntegerProtocolCandidate {
    pub(super) fn from_solution(
        topology: CheckedIntegerProtocolTopology,
        carriers: [CheckedCarrierPlanEvidence; 3],
        operand_facts: [LocalConstantFact; 2],
        constant: PrimitiveConstant,
    ) -> Option<Self> {
        let [(first_storage, first_ty), (second_storage, second_ty)] = topology.check.operands();
        let (result_storage, result_ty) = topology.check.result();
        let expected = [
            (
                first_storage,
                first_ty,
                CheckedCarrierPlanRole::FirstOperand,
            ),
            (
                second_storage,
                second_ty,
                CheckedCarrierPlanRole::SecondOperand,
            ),
            (result_storage, result_ty, CheckedCarrierPlanRole::Result),
        ];
        if constant.ty() != result_ty
            || carriers.iter().zip(expected).any(|(carrier, expected)| {
                carrier.storage() != expected.0
                    || carrier.ty() != expected.1
                    || carrier.role() != expected.2
                    || carrier.check_block() != topology.check_block
            })
            || !carriers[0]
                .loads()
                .contains(&topology.operand_loads[0].value)
            || !carriers[1]
                .loads()
                .contains(&topology.operand_loads[1].value)
            || !carriers[2].loads().contains(&topology.result_reload.value)
            || operand_facts[0].constant().ty() != first_ty
            || operand_facts[1].constant().ty() != second_ty
        {
            return None;
        }

        let operands = std::array::from_fn(|index| CheckedIntegerOperandEvidence {
            storage: carriers[index].storage(),
            source_value: carriers[index].source(),
            constant: operand_facts[index].constant(),
            propagated: operand_facts[index].provenance().category()
                != LocalConstantProvenanceCategory::Literal,
        });
        Some(Self {
            check: topology.check,
            check_block: topology.check_block,
            check_span: topology.check_span,
            success_block: topology.success_block,
            failure_block: topology.failure_block,
            join_block: topology.join_block,
            operands,
            operand_loads: topology.operand_loads,
            result_storage: topology.result_storage,
            result_assignment: topology.result_assignment,
            result_store: topology.result_store,
            result_store_span: topology.result_store_span,
            success_edge_span: topology.success_edge_span,
            result_reload: topology.result_reload,
            constant,
            carriers,
        })
    }

    pub(super) fn has_propagated_operand(&self) -> bool {
        self.operands.iter().any(|operand| operand.propagated)
    }
}

/// Structural result owned by one successful protocol transaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct CheckedIntegerProtocolRewrite {
    pub(super) removed_operand_loads: usize,
}

/// Revalidates one candidate without mutating the callable transaction.
pub(super) fn validate_checked_integer_protocol(
    edit: &MirCallableEdit,
    candidate: &CheckedIntegerProtocolCandidate,
) -> Result<(), MirRewriteError> {
    revalidate(edit, candidate)?;
    Ok(())
}

/// Applies a candidate after its complete callable plan has been validated.
pub(super) fn apply_checked_integer_protocol(
    edit: &mut MirCallableEdit,
    candidate: &CheckedIntegerProtocolCandidate,
) -> Result<CheckedIntegerProtocolRewrite, MirRewriteError> {
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

#[cfg(test)]
pub(super) fn rewrite_checked_integer_protocol(
    edit: &mut MirCallableEdit,
    candidate: &CheckedIntegerProtocolCandidate,
) -> Result<CheckedIntegerProtocolRewrite, MirRewriteError> {
    validate_checked_integer_protocol(edit, candidate)?;
    apply_checked_integer_protocol(edit, candidate)
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
        || !candidate_carriers_match(edit, candidate)
        || !candidate_evaluation_matches(candidate)
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

fn candidate_carriers_match(
    edit: &MirCallableEdit,
    candidate: &CheckedIntegerProtocolCandidate,
) -> bool {
    let [(first_storage, first_ty), (second_storage, second_ty)] = candidate.check.operands();
    let (result_storage, result_ty) = candidate.check.result();
    let expected = [
        (
            first_storage,
            first_ty,
            CheckedCarrierPlanRole::FirstOperand,
        ),
        (
            second_storage,
            second_ty,
            CheckedCarrierPlanRole::SecondOperand,
        ),
        (result_storage, result_ty, CheckedCarrierPlanRole::Result),
    ];

    candidate
        .carriers
        .iter()
        .zip(expected)
        .all(|(carrier, (storage, ty, role))| {
            carrier.storage() == storage
                && carrier.ty() == ty
                && carrier.role() == role
                && carrier.check_block() == candidate.check_block
                && edit
                    .storage(storage)
                    .is_ok_and(|declaration| declaration.ty == ty)
        })
        && candidate.operands[0].storage == first_storage
        && candidate.operands[1].storage == second_storage
        && candidate.operands[0].source_value == candidate.carriers[0].source()
        && candidate.operands[1].source_value == candidate.carriers[1].source()
        && candidate.carriers[0]
            .loads()
            .contains(&candidate.operand_loads[0].value)
        && candidate.carriers[1]
            .loads()
            .contains(&candidate.operand_loads[1].value)
        && candidate.carriers[2]
            .loads()
            .contains(&candidate.result_reload.value)
}

fn candidate_evaluation_matches(candidate: &CheckedIntegerProtocolCandidate) -> bool {
    let evaluation = match candidate.check.operation() {
        CheckedIntegerProtocolOperation::Division(operation) => evaluate_integer_division(
            operation,
            candidate.operands[0].constant,
            candidate.operands[1].constant,
        ),
        CheckedIntegerProtocolOperation::Shift(operation) => evaluate_shift(
            operation,
            candidate.operands[0].constant,
            candidate.operands[1].constant,
        ),
    };
    matches!(evaluation, CheckedIntegerEvaluation::Success(constant) if constant == candidate.constant)
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
