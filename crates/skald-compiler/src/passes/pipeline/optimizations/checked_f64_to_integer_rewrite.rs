//! Atomic rewriting of one revalidated checked floating-to-integer protocol.

use std::collections::HashSet;

use crate::mir::{
    rewrite::{MirCallableEdit, MirRewriteError},
    BlockId, MirInstruction, MirPlace, MirPrimitiveCastRangeCheck, MirRvalueKind, MirTerminator,
    MirType, StorageId, ValueId,
};
use crate::source::Span;

use super::{
    checked_f64_to_integer_evaluation::{evaluate_f64_to_integer, CheckedF64ToIntegerEvaluation},
    checked_f64_to_integer_topology::CheckedF64ToIntegerTopology,
    checked_scalar_topology::{
        edit_storage_write_sites, has_only_predecessor, is_exact_load,
        CheckedScalarInstructionSite, CheckedScalarValueSite,
    },
    local_constant::{
        CheckedCarrierPlanEvidence, CheckedCarrierPlanRole, LocalConstantFact,
        LocalConstantProvenanceCategory,
    },
    primitive_evaluation::PrimitiveConstant,
};

/// Solved value and certified storage evidence for the checked source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CheckedF64ToIntegerSourceEvidence {
    pub(super) storage: StorageId,
    pub(super) source_value: ValueId,
    pub(super) constant: PrimitiveConstant,
    pub(super) propagated: bool,
}

/// Exact immutable source-snapshot input for one checked cast rewrite.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CheckedF64ToIntegerProtocolCandidate {
    pub(super) check: MirPrimitiveCastRangeCheck,
    pub(super) check_block: BlockId,
    pub(super) check_span: Span,
    pub(super) success_block: BlockId,
    pub(super) failure_block: BlockId,
    pub(super) failure_span: Span,
    pub(super) join_block: BlockId,
    pub(super) source: CheckedF64ToIntegerSourceEvidence,
    pub(super) source_load: CheckedScalarValueSite,
    pub(super) result_storage: StorageId,
    pub(super) result_assignment: CheckedScalarValueSite,
    pub(super) result_store: CheckedScalarInstructionSite,
    pub(super) result_store_span: Span,
    pub(super) success_edge_span: Span,
    pub(super) result_reload: CheckedScalarValueSite,
    pub(super) constant: PrimitiveConstant,
    carriers: [CheckedCarrierPlanEvidence; 2],
}

impl CheckedF64ToIntegerProtocolCandidate {
    pub(super) fn from_solution(
        topology: CheckedF64ToIntegerTopology,
        carriers: [CheckedCarrierPlanEvidence; 2],
        source_fact: LocalConstantFact,
        constant: PrimitiveConstant,
    ) -> Option<Self> {
        let (source_storage, source_type) = topology.source();
        let (result_storage, result_type) = topology.result();
        let expected = [
            (source_storage, source_type, CheckedCarrierPlanRole::Source),
            (result_storage, result_type, CheckedCarrierPlanRole::Result),
        ];
        if constant.ty() != result_type
            || source_fact.constant().ty() != MirType::F64
            || carriers.iter().zip(expected).any(|(carrier, expected)| {
                carrier.storage() != expected.0
                    || carrier.ty() != expected.1
                    || carrier.role() != expected.2
                    || carrier.check_block() != topology.check_block
            })
            || !carriers[0].loads().contains(&topology.source_load.value)
            || !carriers[1].loads().contains(&topology.result_reload.value)
            || !matches!(
                evaluate_f64_to_integer(topology.relation(), source_fact.constant()),
                CheckedF64ToIntegerEvaluation::Success(evaluated) if evaluated == constant
            )
        {
            return None;
        }

        Some(Self {
            check: topology.check,
            check_block: topology.check_block,
            check_span: topology.check_span,
            success_block: topology.success_block,
            failure_block: topology.failure_block,
            failure_span: topology.failure_span,
            join_block: topology.join_block,
            source: CheckedF64ToIntegerSourceEvidence {
                storage: source_storage,
                source_value: carriers[0].source(),
                constant: source_fact.constant(),
                propagated: source_fact.provenance().category()
                    != LocalConstantProvenanceCategory::Literal,
            },
            source_load: topology.source_load,
            result_storage,
            result_assignment: topology.result_assignment,
            result_store: topology.result_store,
            result_store_span: topology.result_store_span,
            success_edge_span: topology.success_edge_span,
            result_reload: topology.result_reload,
            constant,
            carriers,
        })
    }

    pub(super) const fn has_propagated_source(&self) -> bool {
        self.source.propagated
    }
}

/// Structural result owned by one successful protocol transaction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct CheckedF64ToIntegerProtocolRewrite {
    pub(super) removed_protocol_values: usize,
}

/// Revalidates one candidate without mutating the callable transaction.
pub(super) fn validate_checked_f64_to_integer_protocol(
    edit: &MirCallableEdit,
    candidate: &CheckedF64ToIntegerProtocolCandidate,
) -> Result<(), MirRewriteError> {
    revalidate(edit, candidate)
}

/// Applies a candidate after its complete callable plan has been validated.
pub(super) fn apply_checked_f64_to_integer_protocol(
    edit: &mut MirCallableEdit,
    candidate: &CheckedF64ToIntegerProtocolCandidate,
) -> Result<CheckedF64ToIntegerProtocolRewrite, MirRewriteError> {
    edit.rewrite_block_terminator(candidate.check_block, |_| {
        Some(MirTerminator::Goto {
            target: candidate.success_block,
            span: candidate.check_span,
        })
    })?;
    edit.rewrite_block_instructions(candidate.success_block, |instructions| {
        let mut result = instructions[candidate.result_assignment.site.instruction].clone();
        let MirInstruction::Assign(assignment) = &mut result else {
            unreachable!("candidate revalidation accepted a checked cast assignment");
        };
        assignment.rvalue.kind = candidate.constant.into_rvalue_kind();
        vec![
            result,
            instructions[candidate.result_store.instruction].clone(),
        ]
    })?;
    edit.remove_value(candidate.source_load.value)?;

    Ok(CheckedF64ToIntegerProtocolRewrite {
        removed_protocol_values: 1,
    })
}

#[cfg(test)]
pub(super) fn rewrite_checked_f64_to_integer_protocol(
    edit: &mut MirCallableEdit,
    candidate: &CheckedF64ToIntegerProtocolCandidate,
) -> Result<CheckedF64ToIntegerProtocolRewrite, MirRewriteError> {
    validate_checked_f64_to_integer_protocol(edit, candidate)?;
    apply_checked_f64_to_integer_protocol(edit, candidate)
}

fn revalidate(
    edit: &MirCallableEdit,
    candidate: &CheckedF64ToIntegerProtocolCandidate,
) -> Result<(), MirRewriteError> {
    validate_live_identities(edit, candidate)?;
    let cfg = edit.local_cfg_facts()?;
    let protected = cfg
        .protected_roots()
        .iter()
        .map(|root| root.block())
        .collect::<HashSet<_>>();

    if !check_matches(edit, candidate)
        || !success_matches(edit, candidate)
        || !failure_matches(edit, candidate)
        || !reload_matches(edit, candidate)
        || !has_only_predecessor(&cfg, candidate.success_block, candidate.check_block)
        || !has_only_predecessor(&cfg, candidate.failure_block, candidate.check_block)
        || !has_only_predecessor(&cfg, candidate.join_block, candidate.success_block)
        || !candidate_carriers_match(edit, candidate)
        || !candidate_evaluation_matches(candidate)
        || edit_storage_write_sites(edit, candidate.result_storage).as_slice()
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
    candidate: &CheckedF64ToIntegerProtocolCandidate,
) -> Result<(), MirRewriteError> {
    for block in [
        candidate.check_block,
        candidate.success_block,
        candidate.failure_block,
        candidate.join_block,
    ] {
        edit.block(block)?;
    }
    for storage in [candidate.source.storage, candidate.result_storage] {
        edit.storage(storage)?;
    }
    for value in [
        candidate.source.source_value,
        candidate.source_load.value,
        candidate.result_assignment.value,
        candidate.result_reload.value,
    ] {
        edit.value(value)?;
    }
    Ok(())
}

fn candidate_carriers_match(
    edit: &MirCallableEdit,
    candidate: &CheckedF64ToIntegerProtocolCandidate,
) -> bool {
    let expected = [
        (
            candidate.check.source,
            MirType::F64,
            CheckedCarrierPlanRole::Source,
        ),
        (
            candidate.check.result,
            candidate.check.relation.result_type(),
            CheckedCarrierPlanRole::Result,
        ),
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
        && candidate.source.storage == candidate.check.source
        && candidate.result_storage == candidate.check.result
        && candidate.source.source_value == candidate.carriers[0].source()
        && candidate.carriers[0]
            .loads()
            .contains(&candidate.source_load.value)
        && candidate.carriers[1]
            .loads()
            .contains(&candidate.result_reload.value)
}

fn candidate_evaluation_matches(candidate: &CheckedF64ToIntegerProtocolCandidate) -> bool {
    matches!(
        evaluate_f64_to_integer(candidate.check.relation, candidate.source.constant),
        CheckedF64ToIntegerEvaluation::Success(constant) if constant == candidate.constant
    )
}

fn check_matches(edit: &MirCallableEdit, candidate: &CheckedF64ToIntegerProtocolCandidate) -> bool {
    edit.block(candidate.check_block).is_ok_and(|block| {
        matches!(
            block.terminator,
            Some(MirTerminator::PrimitiveCastRangeCheck {
                check,
                success_target,
                failure_target,
                span,
            }) if check == candidate.check
                && success_target == candidate.success_block
                && failure_target == candidate.failure_block
                && span == candidate.check_span
        )
    })
}

fn success_matches(
    edit: &MirCallableEdit,
    candidate: &CheckedF64ToIntegerProtocolCandidate,
) -> bool {
    let Ok(block) = edit.block(candidate.success_block) else {
        return false;
    };
    let [MirInstruction::Assign(source), MirInstruction::Assign(result), MirInstruction::Store(store)] =
        block.instructions.as_slice()
    else {
        return false;
    };
    value_site_matches(
        candidate.source_load,
        source.result,
        source.span,
        candidate.success_block,
        0,
    ) && is_exact_load(&source.rvalue.kind, candidate.source.storage)
        && source.rvalue.ty == MirType::F64
        && value_site_matches(
            candidate.result_assignment,
            result.result,
            result.span,
            candidate.success_block,
            1,
        )
        && matches!(
            result.rvalue.kind,
            MirRvalueKind::CheckedF64ToInteger { relation, operand }
                if relation == candidate.check.relation && operand == source.result
        )
        && result.rvalue.ty == candidate.check.relation.result_type()
        && candidate.result_store
            == (CheckedScalarInstructionSite {
                block: candidate.success_block,
                instruction: 2,
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

fn failure_matches(
    edit: &MirCallableEdit,
    candidate: &CheckedF64ToIntegerProtocolCandidate,
) -> bool {
    edit.block(candidate.failure_block).is_ok_and(|block| {
        block.instructions.is_empty()
            && matches!(
                block.terminator,
                Some(MirTerminator::Terminate { reason, span })
                    if reason == candidate.check.relation.failure_reason()
                        && span == candidate.failure_span
            )
    })
}

fn reload_matches(
    edit: &MirCallableEdit,
    candidate: &CheckedF64ToIntegerProtocolCandidate,
) -> bool {
    let Ok(block) = edit.block(candidate.join_block) else {
        return false;
    };
    let Some(MirInstruction::Assign(load)) = block.instructions.first() else {
        return false;
    };
    value_site_matches(
        candidate.result_reload,
        load.result,
        load.span,
        candidate.join_block,
        0,
    ) && is_exact_load(&load.rvalue.kind, candidate.result_storage)
        && load.rvalue.ty == candidate.check.relation.result_type()
}

fn value_site_matches(
    expected: CheckedScalarValueSite,
    value: ValueId,
    span: Span,
    block: BlockId,
    instruction: usize,
) -> bool {
    expected.value == value
        && expected.span == span
        && expected.site == CheckedScalarInstructionSite { block, instruction }
}

fn stale(edit: &MirCallableEdit) -> MirRewriteError {
    MirRewriteError::StaleCallableSnapshot {
        callable: edit.callable(),
        subject: "checked floating-to-integer protocol",
    }
}

#[cfg(test)]
#[path = "checked_f64_to_integer_rewrite/tests.rs"]
mod tests;
