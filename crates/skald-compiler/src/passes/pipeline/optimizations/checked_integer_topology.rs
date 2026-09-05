//! Structural observation of verified checked-integer scalar protocols.
//!
//! Topology discovery deliberately does not decide whether operands are
//! constant. It records one immutable callable-local snapshot which later
//! analyses and the existing narrow constant-candidate adapter may consume.

use std::collections::{HashMap, HashSet};

use crate::{
    mir::{
        checked_scalar_predecessors,
        rewrite::{
            local_cfg_facts_for_definition, MirLocalIdentity, MirLocalIdentitySite,
            MirReferenceFailure, MirRewriteError,
        },
        BlockId, MirDefinitionRef, MirIntegerDivisionOperation, MirIntegerDivisorCheck,
        MirShiftCountCheck, MirShiftOperation, MirStorage, MirStorageKind, MirTerminationReason,
        MirTerminator, MirType, StorageId, ValueId,
    },
    source::Span,
};

use self::shape::{checked_terminator, exact_first_load, success_shape};

mod shape;
pub(super) use shape::storage_write_sites;

/// One instruction's stable location in the current dense callable snapshot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct CheckedIntegerInstructionSite {
    pub(super) block: BlockId,
    pub(super) instruction: usize,
}

/// One value definition and its exact location and source span.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct CheckedIntegerValueSite {
    pub(super) value: ValueId,
    pub(super) site: CheckedIntegerInstructionSite,
    pub(super) span: Span,
}

/// The verifier-owned checked operation and its three scalar carriers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CheckedIntegerProtocolCheck {
    Division(MirIntegerDivisorCheck),
    Shift(MirShiftCountCheck),
}

impl CheckedIntegerProtocolCheck {
    pub(super) const fn operation(self) -> CheckedIntegerProtocolOperation {
        match self {
            Self::Division(check) => CheckedIntegerProtocolOperation::Division(check.operation),
            Self::Shift(check) => CheckedIntegerProtocolOperation::Shift(check.operation),
        }
    }

    pub(super) const fn operands(self) -> [(StorageId, MirType); 2] {
        match self {
            Self::Division(check) => [
                (check.dividend, check.operation.operand_type()),
                (check.divisor, check.operation.operand_type()),
            ],
            Self::Shift(check) => [
                (check.left, check.operation.left_type()),
                (check.count, check.operation.count_type()),
            ],
        }
    }

    pub(super) const fn result(self) -> (StorageId, MirType) {
        match self {
            Self::Division(check) => (check.result, check.operation.result_type()),
            Self::Shift(check) => (check.result, check.operation.result_type()),
        }
    }

    pub(super) const fn failure_reason(self) -> MirTerminationReason {
        match self {
            Self::Division(check) => check.operation.failure_reason(),
            Self::Shift(check) => check.operation.failure_reason(),
        }
    }
}

/// Exact checked arithmetic selected by one protocol.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CheckedIntegerProtocolOperation {
    Division(MirIntegerDivisionOperation),
    Shift(MirShiftOperation),
}

/// Exact immutable structural snapshot of one canonical checked protocol.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CheckedIntegerProtocolTopology {
    pub(super) check: CheckedIntegerProtocolCheck,
    pub(super) check_block: BlockId,
    pub(super) check_span: Span,
    pub(super) success_block: BlockId,
    pub(super) failure_block: BlockId,
    pub(super) join_block: BlockId,
    pub(super) operand_loads: [CheckedIntegerValueSite; 2],
    pub(super) result_storage: StorageId,
    pub(super) result_assignment: CheckedIntegerValueSite,
    pub(super) result_store: CheckedIntegerInstructionSite,
    pub(super) result_store_span: Span,
    pub(super) success_edge_span: Span,
    pub(super) result_reload: CheckedIntegerValueSite,
    pub(super) protected: bool,
}

/// Why a checked terminator does not describe the canonical structural shape.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CheckedIntegerTopologyRejectionReason {
    NonCanonicalTopology,
}

/// One deterministic topology observation in callable block order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum CheckedIntegerTopologyObservation {
    Protocol(Box<CheckedIntegerProtocolTopology>),
    Rejected {
        check_block: BlockId,
        reason: CheckedIntegerTopologyRejectionReason,
    },
}

/// Discovers checked topology without inspecting operand constant provenance.
pub(super) fn observe_checked_integer_topologies(
    definition: MirDefinitionRef<'_>,
) -> Result<Vec<CheckedIntegerTopologyObservation>, MirRewriteError> {
    let cfg = local_cfg_facts_for_definition(definition)?;
    let predecessors = checked_scalar_predecessors(definition);
    let protected = cfg
        .protected_roots()
        .iter()
        .map(|root| root.block())
        .collect::<HashSet<_>>();
    let context = TopologyObservationContext {
        definition,
        predecessors: &predecessors,
        protected: &protected,
    };
    let mut observations = Vec::new();

    for block in &definition.body().blocks {
        let Some((check, success_block, failure_block, check_span)) =
            checked_terminator(block.terminator.as_ref())
        else {
            continue;
        };
        observations.push(
            match observe_topology(
                &context,
                check,
                block.id,
                success_block,
                failure_block,
                check_span,
            )? {
                Some(topology) => CheckedIntegerTopologyObservation::Protocol(Box::new(topology)),
                None => CheckedIntegerTopologyObservation::Rejected {
                    check_block: block.id,
                    reason: CheckedIntegerTopologyRejectionReason::NonCanonicalTopology,
                },
            },
        );
    }
    Ok(observations)
}

struct TopologyObservationContext<'mir, 'facts> {
    definition: MirDefinitionRef<'mir>,
    predecessors: &'facts HashMap<BlockId, HashSet<BlockId>>,
    protected: &'facts HashSet<BlockId>,
}

fn observe_topology(
    context: &TopologyObservationContext<'_, '_>,
    check: CheckedIntegerProtocolCheck,
    check_block: BlockId,
    success_block: BlockId,
    failure_block: BlockId,
    check_span: Span,
) -> Result<Option<CheckedIntegerProtocolTopology>, MirRewriteError> {
    let definition = context.definition;
    let [(first_storage, first_type), (second_storage, second_type)] = check.operands();
    let (result_storage, result_type) = check.result();
    for (storage, expected) in [
        (first_storage, first_type),
        (second_storage, second_type),
        (result_storage, result_type),
    ] {
        let declaration = required_storage(definition, storage, check_block)?;
        if declaration.kind != MirStorageKind::ScalarSpill || declaration.ty != expected {
            return Ok(None);
        }
    }
    if first_storage == second_storage
        || first_storage == result_storage
        || second_storage == result_storage
    {
        return Ok(None);
    }

    let Some(success) = definition.block(success_block) else {
        return Err(invalid_block(definition, success_block, check_block));
    };
    let Some(failure) = definition.block(failure_block) else {
        return Err(invalid_block(definition, failure_block, check_block));
    };
    let Some(shape) = success_shape(success, check) else {
        return Ok(None);
    };
    let Some(join) = definition.block(shape.join_block) else {
        return Err(invalid_block(definition, shape.join_block, success_block));
    };

    if !has_only_predecessor(context.predecessors, success_block, check_block)
        || !has_only_predecessor(context.predecessors, failure_block, check_block)
        || !has_only_predecessor(context.predecessors, shape.join_block, success_block)
        || !failure.instructions.is_empty()
        || !matches!(
            failure.terminator,
            Some(MirTerminator::Terminate { reason, .. }) if reason == check.failure_reason()
        )
        || storage_write_sites(definition, result_storage).as_slice()
            != [CheckedIntegerInstructionSite {
                block: success_block,
                instruction: 3,
            }]
    {
        return Ok(None);
    }

    let Some(result_reload) = exact_first_load(join, result_storage, result_type) else {
        return Ok(None);
    };
    if definition
        .value(shape.operand_loads[0].value)
        .map(|value| value.ty)
        != Some(first_type)
        || definition
            .value(shape.operand_loads[1].value)
            .map(|value| value.ty)
            != Some(second_type)
        || definition
            .value(shape.result_assignment.value)
            .map(|value| value.ty)
            != Some(result_type)
        || definition.value(result_reload.value).map(|value| value.ty) != Some(result_type)
    {
        return Ok(None);
    }

    Ok(Some(CheckedIntegerProtocolTopology {
        check,
        check_block,
        check_span,
        success_block,
        failure_block,
        join_block: shape.join_block,
        operand_loads: shape.operand_loads,
        result_storage,
        result_assignment: shape.result_assignment,
        result_store: shape.result_store,
        result_store_span: shape.result_store_span,
        success_edge_span: shape.success_edge_span,
        result_reload,
        protected: [check_block, success_block, failure_block, shape.join_block]
            .iter()
            .any(|block| context.protected.contains(block)),
    }))
}

fn has_only_predecessor(
    predecessors: &HashMap<BlockId, HashSet<BlockId>>,
    block: BlockId,
    expected: BlockId,
) -> bool {
    predecessors.get(&block) == Some(&HashSet::from([expected]))
}

fn required_storage(
    definition: MirDefinitionRef<'_>,
    storage: StorageId,
    check_block: BlockId,
) -> Result<&MirStorage, MirRewriteError> {
    definition.storage(storage).ok_or_else(|| {
        invalid_reference(
            definition,
            MirLocalIdentity::Storage(storage),
            MirLocalIdentitySite::Terminator(check_block.index()),
        )
    })
}

fn invalid_block(
    definition: MirDefinitionRef<'_>,
    block: BlockId,
    referencing_block: BlockId,
) -> MirRewriteError {
    invalid_reference(
        definition,
        MirLocalIdentity::Block(block),
        MirLocalIdentitySite::Terminator(referencing_block.index()),
    )
}

fn invalid_reference(
    definition: MirDefinitionRef<'_>,
    identity: MirLocalIdentity,
    site: MirLocalIdentitySite,
) -> MirRewriteError {
    let failure = if identity.callable() == definition.callable() {
        MirReferenceFailure::Unknown
    } else {
        MirReferenceFailure::Foreign
    };
    MirRewriteError::InvalidReference {
        expected: definition.callable(),
        identity,
        site,
        failure,
    }
}

#[cfg(test)]
#[path = "checked_integer_topology/tests.rs"]
mod tests;
