//! Constant-candidate adaptation for checked-integer protocol topology.
//!
//! Structural discovery is intentionally independent of constant provenance.
//! This module preserves the existing narrow production behavior by combining
//! that topology with exact literal carrier sources and checked evaluation.

use crate::{
    mir::{
        checked_scalar_dominates,
        rewrite::{
            value_use_census_for_definition, MirLocalIdentity, MirLocalIdentitySite,
            MirReferenceFailure, MirRewriteError, MirValueUseCensus,
        },
        MirDefinitionRef, MirInstruction, MirTerminationReason, MirType, StorageId, ValueId,
    },
    source::Span,
};

pub(super) use super::checked_integer_topology::{
    CheckedIntegerInstructionSite, CheckedIntegerProtocolCheck, CheckedIntegerValueSite,
};
use super::{
    checked_integer_evaluation::{
        evaluate_integer_division, evaluate_shift, CheckedIntegerEvaluation,
    },
    checked_integer_topology::{
        observe_checked_integer_topologies, storage_write_sites, CheckedIntegerProtocolOperation,
        CheckedIntegerProtocolTopology, CheckedIntegerTopologyObservation,
    },
    primitive_evaluation::{evaluate_rvalue, PrimitiveConstant, PrimitiveEvaluation},
};

/// An exact constant assignment and unique dominating store into one operand
/// carrier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct CheckedIntegerConstantCarrier {
    pub(super) storage: StorageId,
    pub(super) constant: PrimitiveConstant,
    pub(super) source_value: ValueId,
    pub(super) source_assignment: CheckedIntegerInstructionSite,
    pub(super) source_span: Span,
    pub(super) store: CheckedIntegerInstructionSite,
    pub(super) store_span: Span,
}

/// Exact immutable snapshot needed to revalidate and rewrite one successful
/// checked-integer protocol.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CheckedIntegerProtocolCandidate {
    pub(super) check: CheckedIntegerProtocolCheck,
    pub(super) check_block: crate::mir::BlockId,
    pub(super) check_span: Span,
    pub(super) success_block: crate::mir::BlockId,
    pub(super) failure_block: crate::mir::BlockId,
    pub(super) join_block: crate::mir::BlockId,
    pub(super) operands: [CheckedIntegerConstantCarrier; 2],
    pub(super) operand_loads: [CheckedIntegerValueSite; 2],
    pub(super) result_storage: StorageId,
    pub(super) result_assignment: CheckedIntegerValueSite,
    pub(super) result_store: CheckedIntegerInstructionSite,
    pub(super) result_store_span: Span,
    pub(super) success_edge_span: Span,
    pub(super) result_reload: CheckedIntegerValueSite,
    pub(super) constant: PrimitiveConstant,
}

/// Why one structurally present checked protocol is not currently eligible.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CheckedIntegerProtocolRejectionReason {
    StaticFailure(MirTerminationReason),
    DynamicOperand,
    NonCanonicalTopology,
    ProtectedTopology,
    UnsupportedOperation,
}

/// One deterministic constant-candidate observation in callable block order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum CheckedIntegerProtocolObservation {
    Candidate(Box<CheckedIntegerProtocolCandidate>),
    Rejected {
        check_block: crate::mir::BlockId,
        reason: CheckedIntegerProtocolRejectionReason,
    },
}

/// Preserves the existing checked-folding eligibility contract over the new
/// provenance-independent structural observations.
pub(super) fn observe_checked_integer_protocols(
    definition: MirDefinitionRef<'_>,
) -> Result<Vec<CheckedIntegerProtocolObservation>, MirRewriteError> {
    let value_census = value_use_census_for_definition(definition)?;
    observe_checked_integer_topologies(definition)?
        .into_iter()
        .map(|observation| match observation {
            CheckedIntegerTopologyObservation::Protocol(topology) => {
                let check_block = topology.check_block;
                adapt_topology(definition, &value_census, *topology).map(|result| match result {
                    Ok(candidate) => {
                        CheckedIntegerProtocolObservation::Candidate(Box::new(candidate))
                    }
                    Err(reason) => CheckedIntegerProtocolObservation::Rejected {
                        check_block,
                        reason,
                    },
                })
            }
            CheckedIntegerTopologyObservation::Rejected { check_block, .. } => {
                Ok(CheckedIntegerProtocolObservation::Rejected {
                    check_block,
                    reason: CheckedIntegerProtocolRejectionReason::NonCanonicalTopology,
                })
            }
        })
        .collect()
}

fn adapt_topology(
    definition: MirDefinitionRef<'_>,
    value_census: &MirValueUseCensus,
    topology: CheckedIntegerProtocolTopology,
) -> Result<
    Result<CheckedIntegerProtocolCandidate, CheckedIntegerProtocolRejectionReason>,
    MirRewriteError,
> {
    if topology.protected {
        return Ok(Err(
            CheckedIntegerProtocolRejectionReason::ProtectedTopology,
        ));
    }

    let [(first_storage, first_type), (second_storage, second_type)] = topology.check.operands();
    let first = match constant_carrier_source(
        definition,
        value_census,
        first_storage,
        first_type,
        topology.check_block,
    )? {
        Some(source) => source,
        None => return Ok(Err(CheckedIntegerProtocolRejectionReason::DynamicOperand)),
    };
    let second = match constant_carrier_source(
        definition,
        value_census,
        second_storage,
        second_type,
        topology.check_block,
    )? {
        Some(source) => source,
        None => return Ok(Err(CheckedIntegerProtocolRejectionReason::DynamicOperand)),
    };
    let constant = match evaluate_operation(
        topology.check.operation(),
        [first.constant, second.constant],
    ) {
        CheckedIntegerEvaluation::Success(constant) => constant,
        CheckedIntegerEvaluation::Failure(reason) => {
            return Ok(Err(CheckedIntegerProtocolRejectionReason::StaticFailure(
                reason,
            )));
        }
        CheckedIntegerEvaluation::Unsupported => {
            return Ok(Err(
                CheckedIntegerProtocolRejectionReason::UnsupportedOperation,
            ));
        }
    };

    Ok(Ok(CheckedIntegerProtocolCandidate {
        check: topology.check,
        check_block: topology.check_block,
        check_span: topology.check_span,
        success_block: topology.success_block,
        failure_block: topology.failure_block,
        join_block: topology.join_block,
        operands: [first, second],
        operand_loads: topology.operand_loads,
        result_storage: topology.result_storage,
        result_assignment: topology.result_assignment,
        result_store: topology.result_store,
        result_store_span: topology.result_store_span,
        success_edge_span: topology.success_edge_span,
        result_reload: topology.result_reload,
        constant,
    }))
}

fn evaluate_operation(
    operation: CheckedIntegerProtocolOperation,
    operands: [PrimitiveConstant; 2],
) -> CheckedIntegerEvaluation {
    match operation {
        CheckedIntegerProtocolOperation::Division(operation) => {
            evaluate_integer_division(operation, operands[0], operands[1])
        }
        CheckedIntegerProtocolOperation::Shift(operation) => {
            evaluate_shift(operation, operands[0], operands[1])
        }
    }
}

fn constant_carrier_source(
    definition: MirDefinitionRef<'_>,
    census: &MirValueUseCensus,
    storage: StorageId,
    expected_type: MirType,
    check_block: crate::mir::BlockId,
) -> Result<Option<CheckedIntegerConstantCarrier>, MirRewriteError> {
    let writes = storage_write_sites(definition, storage);
    let [store_site] = writes.as_slice() else {
        return Ok(None);
    };
    if !checked_scalar_dominates(definition, store_site.block, check_block) {
        return Ok(None);
    }
    let block = definition
        .block(store_site.block)
        .expect("store scan returns a block from this definition");
    let MirInstruction::Store(store) = &block.instructions[store_site.instruction] else {
        unreachable!("storage write scan returns only stores");
    };
    if store.authorization.is_some() || store.final_authorization.is_some() {
        return Ok(None);
    }

    let Some(source_entry) = census.get(store.value) else {
        return Err(invalid_value(definition, store.value, *store_site));
    };
    let Some(MirLocalIdentitySite::Instruction { block, instruction }) = source_entry.definition()
    else {
        return Ok(None);
    };
    let source_site = CheckedIntegerInstructionSite {
        block: crate::mir::BlockId::new(definition.callable(), block),
        instruction,
    };
    if !checked_scalar_dominates(definition, source_site.block, store_site.block)
        || (source_site.block == store_site.block
            && source_site.instruction >= store_site.instruction)
    {
        return Ok(None);
    }
    let source_block = definition
        .block(source_site.block)
        .expect("value census returns a block from this definition");
    let Some(MirInstruction::Assign(assignment)) = source_block.instructions.get(instruction)
    else {
        return Ok(None);
    };
    let PrimitiveEvaluation::Constant(constant) =
        evaluate_rvalue(&assignment.rvalue.kind, |_| None)
    else {
        return Ok(None);
    };
    if assignment.result != store.value
        || assignment.rvalue.ty != expected_type
        || definition.value(store.value).map(|value| value.ty) != Some(expected_type)
        || constant.ty() != expected_type
    {
        return Ok(None);
    }

    Ok(Some(CheckedIntegerConstantCarrier {
        storage,
        constant,
        source_value: store.value,
        source_assignment: source_site,
        source_span: assignment.span,
        store: *store_site,
        store_span: store.span,
    }))
}

fn invalid_value(
    definition: MirDefinitionRef<'_>,
    value: ValueId,
    site: CheckedIntegerInstructionSite,
) -> MirRewriteError {
    let identity = MirLocalIdentity::Value(value);
    let failure = if identity.callable() == definition.callable() {
        MirReferenceFailure::Unknown
    } else {
        MirReferenceFailure::Foreign
    };
    MirRewriteError::InvalidReference {
        expected: definition.callable(),
        identity,
        site: MirLocalIdentitySite::Instruction {
            block: site.block.index(),
            instruction: site.instruction,
        },
        failure,
    }
}

#[cfg(test)]
#[path = "checked_integer_protocol/tests.rs"]
mod tests;
