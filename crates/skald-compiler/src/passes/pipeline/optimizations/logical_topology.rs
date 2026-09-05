//! Structural observation of verified short-circuit logical control flow.
//!
//! The observation owns identities and spans from one callable snapshot. It
//! deliberately records no constant facts and exposes no mutation surface.

#![allow(dead_code)] // CLR0 establishes this facade before the CLR2 solver consumes it.

use std::collections::{HashMap, HashSet};

use crate::{
    mir::{
        rewrite::{local_cfg_facts_for_definition, MirLocalIdentity, MirRewriteError},
        BlockId, MirDefinitionRef, MirInstruction, MirLogicalExpression, MirLogicalOperation,
        MirPathCondition, MirPlace, MirRvalueKind, MirStorageKind, MirTerminator, MirType,
        PathConditionId, StorageId, ValueId,
    },
    source::Span,
};

use self::identity::{invalid_reference, validate_references};

mod identity;

/// Exact immutable topology of one verified short-circuit expression.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct LogicalProtocolTopology {
    pub(super) record_index: usize,
    pub(super) operation: MirLogicalOperation,
    pub(super) condition: PathConditionId,
    pub(super) parent_condition: Option<PathConditionId>,
    pub(super) activation: StorageId,
    pub(super) active_predecessor: BlockId,
    pub(super) inactive_predecessor: BlockId,
    pub(super) result: StorageId,
    pub(super) left_result: ValueId,
    pub(super) split: BlockId,
    pub(super) selection: BlockId,
    pub(super) right_entry: BlockId,
    pub(super) right_exit: BlockId,
    pub(super) right_result: ValueId,
    pub(super) short: BlockId,
    pub(super) join: BlockId,
    pub(super) selected_result: ValueId,
    pub(super) logical_span: Span,
    pub(super) condition_span: Span,
}

/// Why a logical record could not be interpreted as canonical topology.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum LogicalTopologyRejectionReason {
    DuplicateCondition,
    MismatchedPathCondition,
    NonCanonicalTopology,
}

/// One deterministic observation in logical-record order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum LogicalTopologyObservation {
    Protocol(Box<LogicalProtocolTopology>),
    Rejected {
        record_index: usize,
        reason: LogicalTopologyRejectionReason,
    },
}

/// Observes verified logical topology without requiring either operand to be
/// constant and without retaining references into MIR.
pub(super) fn observe_logical_topologies(
    definition: MirDefinitionRef<'_>,
) -> Result<Vec<LogicalTopologyObservation>, MirRewriteError> {
    // Reuse the exhaustive MIR identity/CFG validation boundary before
    // interpreting proof-record relationships.
    let _cfg = local_cfg_facts_for_definition(definition)?;
    let predecessors = predecessors(definition);
    let mut claimed_conditions = HashSet::new();
    let mut observations = Vec::with_capacity(definition.logical_expressions().len());

    for (record_index, logical) in definition.logical_expressions().iter().enumerate() {
        if !claimed_conditions.insert(logical.condition) {
            observations.push(LogicalTopologyObservation::Rejected {
                record_index,
                reason: LogicalTopologyRejectionReason::DuplicateCondition,
            });
            continue;
        }

        let Some(condition) = definition.path_condition(logical.condition) else {
            return Err(invalid_reference(
                definition,
                MirLocalIdentity::PathCondition(logical.condition),
                record_index,
            ));
        };
        validate_references(definition, logical, condition, record_index)?;

        let reason = if condition.merge != logical.selection {
            Some(LogicalTopologyRejectionReason::MismatchedPathCondition)
        } else if !is_canonical_logical_topology(definition, logical, condition, &predecessors) {
            Some(LogicalTopologyRejectionReason::NonCanonicalTopology)
        } else {
            None
        };

        observations.push(match reason {
            Some(reason) => LogicalTopologyObservation::Rejected {
                record_index,
                reason,
            },
            None => LogicalTopologyObservation::Protocol(Box::new(LogicalProtocolTopology {
                record_index,
                operation: logical.operation,
                condition: logical.condition,
                parent_condition: condition.parent,
                activation: condition.activation,
                active_predecessor: condition.active_predecessor,
                inactive_predecessor: condition.inactive_predecessor,
                result: logical.result,
                left_result: logical.left_result,
                split: logical.split,
                selection: logical.selection,
                right_entry: logical.right_entry,
                right_exit: logical.right_exit,
                right_result: logical.right_result,
                short: logical.short,
                join: logical.join,
                selected_result: logical.selected_result,
                logical_span: logical.span,
                condition_span: condition.span,
            })),
        });
    }

    Ok(observations)
}

fn is_canonical_logical_topology(
    definition: MirDefinitionRef<'_>,
    logical: &MirLogicalExpression,
    condition: &MirPathCondition,
    predecessors: &HashMap<BlockId, HashSet<BlockId>>,
) -> bool {
    let Some(result) = definition.storage(logical.result) else {
        return false;
    };
    let Some(activation) = definition.storage(condition.activation) else {
        return false;
    };
    if result.kind != MirStorageKind::ScalarSpill
        || result.ty != MirType::Bool
        || activation.kind != MirStorageKind::PathCondition
        || activation.ty != MirType::Bool
        || [
            logical.left_result,
            logical.right_result,
            logical.selected_result,
        ]
        .into_iter()
        .any(|value| definition.value(value).map(|value| value.ty) != Some(MirType::Bool))
    {
        return false;
    }

    let expected_split_targets = match logical.operation {
        MirLogicalOperation::And => (condition.active_predecessor, condition.inactive_predecessor),
        MirLogicalOperation::Or => (condition.inactive_predecessor, condition.active_predecessor),
    };
    let Some(split) = definition.block(logical.split) else {
        return false;
    };
    if !matches!(
        split.terminator,
        Some(MirTerminator::Branch {
            condition,
            true_target,
            false_target,
            ..
        }) if condition == logical.left_result
            && (true_target, false_target) == expected_split_targets
    ) {
        return false;
    }

    if !selection_predecessor(
        definition,
        condition.active_predecessor,
        condition.activation,
        condition.merge,
        true,
    ) || !selection_predecessor(
        definition,
        condition.inactive_predecessor,
        condition.activation,
        condition.merge,
        false,
    ) || predecessors.get(&condition.merge)
        != Some(&HashSet::from([
            condition.active_predecessor,
            condition.inactive_predecessor,
        ]))
    {
        return false;
    }

    if !selection_block(definition, logical, condition)
        || !result_predecessor(
            definition,
            logical.short,
            logical.join,
            logical.result,
            None,
            Some(logical.operation.fixed_short_result()),
        )
        || !result_predecessor(
            definition,
            logical.right_exit,
            logical.join,
            logical.result,
            Some(logical.right_result),
            None,
        )
        || predecessors.get(&logical.join)
            != Some(&HashSet::from([logical.short, logical.right_exit]))
        || !join_loads_selected_result(definition, logical)
    {
        return false;
    }

    result_write_blocks(definition, logical.result)
        == [logical.right_exit, logical.short]
            .into_iter()
            .collect::<HashSet<_>>()
}

fn selection_predecessor(
    definition: MirDefinitionRef<'_>,
    block: BlockId,
    activation: StorageId,
    selection: BlockId,
    expected: bool,
) -> bool {
    let Some(block) = definition.block(block) else {
        return false;
    };
    let Some(MirInstruction::Store(store)) = block.instructions.last() else {
        return false;
    };
    store.destination == MirPlace::base(activation)
        && constant_bool(block, store.value) == Some(expected)
        && matches!(block.terminator, Some(MirTerminator::Goto { target, .. }) if target == selection)
}

fn selection_block(
    definition: MirDefinitionRef<'_>,
    logical: &MirLogicalExpression,
    condition: &MirPathCondition,
) -> bool {
    let Some(block) = definition.block(logical.selection) else {
        return false;
    };
    let read = block.instructions.iter().find_map(|instruction| {
        let MirInstruction::Assign(assignment) = instruction else {
            return None;
        };
        matches!(
            assignment.rvalue.kind,
            MirRvalueKind::PathCondition(read)
                if read.condition == logical.condition
                    && read.activation == condition.activation
        )
        .then_some(assignment.result)
    });
    matches!(
        (read, &block.terminator),
        (
            Some(read),
            Some(MirTerminator::Branch {
                condition,
                true_target,
                false_target,
                ..
            })
        ) if *condition == read
            && *true_target == logical.right_entry
            && *false_target == logical.short
    )
}

fn result_predecessor(
    definition: MirDefinitionRef<'_>,
    block: BlockId,
    join: BlockId,
    result: StorageId,
    expected_value: Option<ValueId>,
    fixed: Option<bool>,
) -> bool {
    let Some(block) = definition.block(block) else {
        return false;
    };
    let Some(MirInstruction::Store(store)) = block.instructions.last() else {
        return false;
    };
    store.destination == MirPlace::base(result)
        && expected_value.is_none_or(|expected| store.value == expected)
        && fixed.is_none_or(|expected| constant_bool(block, store.value) == Some(expected))
        && matches!(block.terminator, Some(MirTerminator::Goto { target, .. }) if target == join)
}

fn join_loads_selected_result(
    definition: MirDefinitionRef<'_>,
    logical: &MirLogicalExpression,
) -> bool {
    definition.block(logical.join).is_some_and(|block| {
        block.instructions.iter().any(|instruction| {
            matches!(
                instruction,
                MirInstruction::Assign(assignment)
                    if assignment.result == logical.selected_result
                        && assignment.rvalue.ty == MirType::Bool
                        && matches!(
                            assignment.rvalue.kind,
                            MirRvalueKind::Load(ref place)
                                if *place == MirPlace::base(logical.result)
                        )
            )
        })
    })
}

fn result_write_blocks(definition: MirDefinitionRef<'_>, result: StorageId) -> HashSet<BlockId> {
    definition
        .body()
        .blocks
        .iter()
        .flat_map(|block| {
            block.instructions.iter().filter_map(move |instruction| {
                matches!(
                    instruction,
                    MirInstruction::Store(store)
                        if store.destination == MirPlace::base(result)
                )
                .then_some(block.id)
            })
        })
        .collect()
}

fn constant_bool(block: &crate::mir::MirBasicBlock, value: ValueId) -> Option<bool> {
    block
        .instructions
        .iter()
        .find_map(|instruction| match instruction {
            MirInstruction::Assign(assignment) if assignment.result == value => {
                match assignment.rvalue.kind {
                    MirRvalueKind::ConstantBool(value) => Some(value),
                    _ => None,
                }
            }
            _ => None,
        })
}

fn predecessors(definition: MirDefinitionRef<'_>) -> HashMap<BlockId, HashSet<BlockId>> {
    let mut predecessors: HashMap<_, HashSet<_>> = HashMap::new();
    for block in &definition.body().blocks {
        for target in block.terminator.iter().flat_map(MirTerminator::successors) {
            predecessors.entry(target).or_default().insert(block.id);
        }
    }
    predecessors
}

#[cfg(test)]
#[path = "logical_topology/tests.rs"]
mod tests;
