//! Structural observation of verified checked floating-to-integer protocols.
//!
//! Discovery records one owned callable-local snapshot without deciding
//! whether the source is constant or whether the protocol should be rewritten.

use std::collections::{HashMap, HashSet};

use crate::{
    mir::{
        checked_scalar_predecessors,
        rewrite::{local_cfg_facts_for_definition, MirRewriteError},
        BlockId, MirDefinitionRef, MirF64ToIntegerRange, MirPrimitiveCastRangeCheck,
        MirStorageKind, MirTerminationReason, MirTerminator, MirType, StorageId,
    },
    source::Span,
};

use self::shape::{checked_terminator, success_shape};
use super::checked_scalar_topology::{
    exact_first_load, has_only_predecessor, invalid_block, required_storage, storage_write_sites,
    CheckedScalarInstructionSite, CheckedScalarValueSite,
};

mod shape;

/// Exact immutable structural snapshot of one checked floating cast.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CheckedF64ToIntegerTopology {
    pub(super) check: MirPrimitiveCastRangeCheck,
    pub(super) check_block: BlockId,
    pub(super) check_span: Span,
    pub(super) success_block: BlockId,
    pub(super) failure_block: BlockId,
    pub(super) failure_span: Span,
    pub(super) join_block: BlockId,
    pub(super) source_load: CheckedScalarValueSite,
    pub(super) result_assignment: CheckedScalarValueSite,
    pub(super) result_store: CheckedScalarInstructionSite,
    pub(super) result_store_span: Span,
    pub(super) success_edge_span: Span,
    pub(super) result_reload: CheckedScalarValueSite,
    pub(super) protected: bool,
}

impl CheckedF64ToIntegerTopology {
    pub(super) const fn relation(&self) -> MirF64ToIntegerRange {
        self.check.relation
    }

    pub(super) const fn source(&self) -> (StorageId, MirType) {
        (self.check.source, MirType::F64)
    }

    pub(super) const fn result(&self) -> (StorageId, MirType) {
        (self.check.result, self.check.relation.result_type())
    }
}

/// Why a checked cast terminator does not describe the canonical shape.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CheckedF64ToIntegerTopologyRejectionReason {
    NonCanonicalTopology,
}

/// One deterministic topology observation in callable block order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum CheckedF64ToIntegerTopologyObservation {
    Protocol(Box<CheckedF64ToIntegerTopology>),
    Rejected {
        check_block: BlockId,
        reason: CheckedF64ToIntegerTopologyRejectionReason,
    },
}

/// Discovers checked floating-cast topology without inspecting source facts.
pub(super) fn observe_checked_f64_to_integer_topologies(
    definition: MirDefinitionRef<'_>,
) -> Result<Vec<CheckedF64ToIntegerTopologyObservation>, MirRewriteError> {
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
                Some(topology) => {
                    CheckedF64ToIntegerTopologyObservation::Protocol(Box::new(topology))
                }
                None => CheckedF64ToIntegerTopologyObservation::Rejected {
                    check_block: block.id,
                    reason: CheckedF64ToIntegerTopologyRejectionReason::NonCanonicalTopology,
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
    check: MirPrimitiveCastRangeCheck,
    check_block: BlockId,
    success_block: BlockId,
    failure_block: BlockId,
    check_span: Span,
) -> Result<Option<CheckedF64ToIntegerTopology>, MirRewriteError> {
    let definition = context.definition;
    let source = required_storage(definition, check.source, check_block)?;
    let result = required_storage(definition, check.result, check_block)?;
    let result_type = check.relation.result_type();
    if source.kind != MirStorageKind::ScalarSpill
        || source.ty != MirType::F64
        || result.kind != MirStorageKind::ScalarSpill
        || result.ty != result_type
        || check.source == check.result
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
    let Some(MirTerminator::Terminate {
        reason: MirTerminationReason::PrimitiveCastOutOfRange,
        span: failure_span,
    }) = failure.terminator
    else {
        return Ok(None);
    };

    if !has_only_predecessor(context.predecessors, success_block, check_block)
        || !has_only_predecessor(context.predecessors, failure_block, check_block)
        || !has_only_predecessor(context.predecessors, shape.join_block, success_block)
        || !failure.instructions.is_empty()
        || storage_write_sites(definition, check.result).as_slice()
            != [CheckedScalarInstructionSite {
                block: success_block,
                instruction: 2,
            }]
    {
        return Ok(None);
    }

    let Some(result_reload) = exact_first_load(join, check.result, result_type) else {
        return Ok(None);
    };
    if definition
        .value(shape.source_load.value)
        .map(|value| value.ty)
        != Some(MirType::F64)
        || definition
            .value(shape.result_assignment.value)
            .map(|value| value.ty)
            != Some(result_type)
        || definition.value(result_reload.value).map(|value| value.ty) != Some(result_type)
    {
        return Ok(None);
    }

    Ok(Some(CheckedF64ToIntegerTopology {
        check,
        check_block,
        check_span,
        success_block,
        failure_block,
        failure_span,
        join_block: shape.join_block,
        source_load: shape.source_load,
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

#[cfg(test)]
#[path = "checked_f64_to_integer_topology/tests.rs"]
mod tests;
