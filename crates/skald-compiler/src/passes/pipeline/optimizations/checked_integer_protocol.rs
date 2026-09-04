//! Immutable discovery of verified checked-integer scalar protocols.
//!
//! This module deliberately observes dense final MIR without retaining facts
//! across rewrites. Its candidates are exact snapshots for the later atomic
//! rewrite owner, not a general storage or dataflow analysis.

use std::collections::{HashMap, HashSet};

use crate::{
    mir::{
        checked_scalar_dominates, checked_scalar_predecessors,
        rewrite::{
            local_cfg_facts_for_definition, value_use_census_for_definition, MirLocalIdentity,
            MirLocalIdentitySite, MirReferenceFailure, MirRewriteError, MirValueUseCensus,
        },
        BlockId, MirDefinitionRef, MirInstruction, MirIntegerDivisionOperation,
        MirIntegerDivisorCheck, MirPlace, MirRvalueKind, MirShiftCountCheck, MirShiftOperation,
        MirStorage, MirStorageKind, MirTerminationReason, MirTerminator, MirType, StorageId,
        ValueId,
    },
    source::Span,
};

use super::{
    checked_integer_evaluation::{
        evaluate_integer_division, evaluate_shift, CheckedIntegerEvaluation,
    },
    primitive_evaluation::{evaluate_rvalue, PrimitiveConstant, PrimitiveEvaluation},
};

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

/// The verifier-owned checked operation and its three scalar carriers.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CheckedIntegerProtocolCheck {
    Division(MirIntegerDivisorCheck),
    Shift(MirShiftCountCheck),
}

impl CheckedIntegerProtocolCheck {
    const fn operation(self) -> CheckedIntegerProtocolOperation {
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

impl CheckedIntegerProtocolOperation {
    fn evaluate(self, operands: [PrimitiveConstant; 2]) -> CheckedIntegerEvaluation {
        match self {
            Self::Division(operation) => {
                evaluate_integer_division(operation, operands[0], operands[1])
            }
            Self::Shift(operation) => evaluate_shift(operation, operands[0], operands[1]),
        }
    }
}

/// Exact immutable snapshot needed to revalidate and rewrite one successful
/// checked-integer protocol.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CheckedIntegerProtocolCandidate {
    pub(super) check: CheckedIntegerProtocolCheck,
    pub(super) check_block: BlockId,
    pub(super) check_span: Span,
    pub(super) success_block: BlockId,
    pub(super) failure_block: BlockId,
    pub(super) join_block: BlockId,
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

/// One deterministic observation in callable block order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum CheckedIntegerProtocolObservation {
    Candidate(Box<CheckedIntegerProtocolCandidate>),
    Rejected {
        check_block: BlockId,
        reason: CheckedIntegerProtocolRejectionReason,
    },
}

/// Discovers exact checked-integer candidates in one borrowed verified
/// callable without cloning or mutating MIR.
pub(super) fn observe_checked_integer_protocols(
    definition: MirDefinitionRef<'_>,
) -> Result<Vec<CheckedIntegerProtocolObservation>, MirRewriteError> {
    let cfg = local_cfg_facts_for_definition(definition)?;
    let value_census = value_use_census_for_definition(definition)?;
    let predecessors = checked_scalar_predecessors(definition);
    let protected = cfg
        .protected_roots()
        .iter()
        .map(|root| root.block())
        .collect::<HashSet<_>>();
    let context = ProtocolObservationContext {
        definition,
        value_census: &value_census,
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
        let result = observe_protocol(
            &context,
            check,
            block.id,
            success_block,
            failure_block,
            check_span,
        )?;
        observations.push(match result {
            Ok(candidate) => CheckedIntegerProtocolObservation::Candidate(Box::new(candidate)),
            Err(reason) => CheckedIntegerProtocolObservation::Rejected {
                check_block: block.id,
                reason,
            },
        });
    }
    Ok(observations)
}

struct ProtocolObservationContext<'mir, 'facts> {
    definition: MirDefinitionRef<'mir>,
    value_census: &'facts MirValueUseCensus,
    predecessors: &'facts HashMap<BlockId, HashSet<BlockId>>,
    protected: &'facts HashSet<BlockId>,
}

fn observe_protocol(
    context: &ProtocolObservationContext<'_, '_>,
    check: CheckedIntegerProtocolCheck,
    check_block: BlockId,
    success_block: BlockId,
    failure_block: BlockId,
    check_span: Span,
) -> Result<
    Result<CheckedIntegerProtocolCandidate, CheckedIntegerProtocolRejectionReason>,
    MirRewriteError,
> {
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
            return Ok(Err(
                CheckedIntegerProtocolRejectionReason::NonCanonicalTopology,
            ));
        }
    }
    if first_storage == second_storage
        || first_storage == result_storage
        || second_storage == result_storage
    {
        return Ok(Err(
            CheckedIntegerProtocolRejectionReason::NonCanonicalTopology,
        ));
    }

    let Some(success) = definition.block(success_block) else {
        return Err(invalid_block(definition, success_block, check_block));
    };
    let Some(failure) = definition.block(failure_block) else {
        return Err(invalid_block(definition, failure_block, check_block));
    };
    let Some(shape) = success_shape(success, check) else {
        return Ok(Err(
            CheckedIntegerProtocolRejectionReason::NonCanonicalTopology,
        ));
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
        return Ok(Err(
            CheckedIntegerProtocolRejectionReason::NonCanonicalTopology,
        ));
    }

    let Some(result_reload) = exact_first_load(join, result_storage, result_type) else {
        return Ok(Err(
            CheckedIntegerProtocolRejectionReason::NonCanonicalTopology,
        ));
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
        return Ok(Err(
            CheckedIntegerProtocolRejectionReason::NonCanonicalTopology,
        ));
    }
    if context.protected.contains(&check_block)
        || context.protected.contains(&success_block)
        || context.protected.contains(&failure_block)
        || context.protected.contains(&shape.join_block)
    {
        return Ok(Err(
            CheckedIntegerProtocolRejectionReason::ProtectedTopology,
        ));
    }

    let first = match constant_carrier_source(
        definition,
        context.value_census,
        first_storage,
        first_type,
        check_block,
    )? {
        Some(source) => source,
        None => return Ok(Err(CheckedIntegerProtocolRejectionReason::DynamicOperand)),
    };
    let second = match constant_carrier_source(
        definition,
        context.value_census,
        second_storage,
        second_type,
        check_block,
    )? {
        Some(source) => source,
        None => return Ok(Err(CheckedIntegerProtocolRejectionReason::DynamicOperand)),
    };
    let constant = match check
        .operation()
        .evaluate([first.constant, second.constant])
    {
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
        check,
        check_block,
        check_span,
        success_block,
        failure_block,
        join_block: shape.join_block,
        operands: [first, second],
        operand_loads: shape.operand_loads,
        result_storage,
        result_assignment: shape.result_assignment,
        result_store: shape.result_store,
        result_store_span: shape.result_store_span,
        success_edge_span: shape.success_edge_span,
        result_reload,
        constant,
    }))
}

#[derive(Clone, Copy)]
struct SuccessShape {
    join_block: BlockId,
    operand_loads: [CheckedIntegerValueSite; 2],
    result_assignment: CheckedIntegerValueSite,
    result_store: CheckedIntegerInstructionSite,
    result_store_span: Span,
    success_edge_span: Span,
}

fn success_shape(
    block: &crate::mir::MirBasicBlock,
    check: CheckedIntegerProtocolCheck,
) -> Option<SuccessShape> {
    let [MirInstruction::Assign(first), MirInstruction::Assign(second), MirInstruction::Assign(result), MirInstruction::Store(store)] =
        block.instructions.as_slice()
    else {
        return None;
    };
    let [(first_storage, first_type), (second_storage, second_type)] = check.operands();
    let (result_storage, result_type) = check.result();
    if !is_exact_load(&first.rvalue.kind, first_storage)
        || first.rvalue.ty != first_type
        || !is_exact_load(&second.rvalue.kind, second_storage)
        || second.rvalue.ty != second_type
        || store.destination != MirPlace::base(result_storage)
        || store.value != result.result
        || store.authorization.is_some()
        || store.final_authorization.is_some()
        || result.rvalue.ty != result_type
    {
        return None;
    }
    let operation_matches = match (check, &result.rvalue.kind) {
        (
            CheckedIntegerProtocolCheck::Division(check),
            MirRvalueKind::IntegerDivision {
                operation,
                dividend,
                divisor,
            },
        ) => {
            *operation == check.operation && *dividend == first.result && *divisor == second.result
        }
        (
            CheckedIntegerProtocolCheck::Shift(check),
            MirRvalueKind::Shift {
                operation,
                left,
                count,
            },
        ) => *operation == check.operation && *left == first.result && *count == second.result,
        _ => false,
    };
    if !operation_matches {
        return None;
    }
    let Some(MirTerminator::Goto { target, span }) = block.terminator else {
        return None;
    };
    Some(SuccessShape {
        join_block: target,
        operand_loads: [
            CheckedIntegerValueSite {
                value: first.result,
                site: CheckedIntegerInstructionSite {
                    block: block.id,
                    instruction: 0,
                },
                span: first.span,
            },
            CheckedIntegerValueSite {
                value: second.result,
                site: CheckedIntegerInstructionSite {
                    block: block.id,
                    instruction: 1,
                },
                span: second.span,
            },
        ],
        result_assignment: CheckedIntegerValueSite {
            value: result.result,
            site: CheckedIntegerInstructionSite {
                block: block.id,
                instruction: 2,
            },
            span: result.span,
        },
        result_store: CheckedIntegerInstructionSite {
            block: block.id,
            instruction: 3,
        },
        result_store_span: store.span,
        success_edge_span: span,
    })
}

fn constant_carrier_source(
    definition: MirDefinitionRef<'_>,
    census: &MirValueUseCensus,
    storage: StorageId,
    expected_type: MirType,
    check_block: BlockId,
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
        block: BlockId::new(definition.callable(), block),
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

fn checked_terminator(
    terminator: Option<&MirTerminator>,
) -> Option<(CheckedIntegerProtocolCheck, BlockId, BlockId, Span)> {
    match terminator? {
        MirTerminator::IntegerDivisorCheck {
            check,
            success_target,
            failure_target,
            span,
        } => Some((
            CheckedIntegerProtocolCheck::Division(*check),
            *success_target,
            *failure_target,
            *span,
        )),
        MirTerminator::ShiftCountCheck {
            check,
            success_target,
            failure_target,
            span,
        } => Some((
            CheckedIntegerProtocolCheck::Shift(*check),
            *success_target,
            *failure_target,
            *span,
        )),
        _ => None,
    }
}

fn exact_first_load(
    block: &crate::mir::MirBasicBlock,
    storage: StorageId,
    ty: MirType,
) -> Option<CheckedIntegerValueSite> {
    let Some(MirInstruction::Assign(load)) = block.instructions.first() else {
        return None;
    };
    (is_exact_load(&load.rvalue.kind, storage) && load.rvalue.ty == ty).then_some(
        CheckedIntegerValueSite {
            value: load.result,
            site: CheckedIntegerInstructionSite {
                block: block.id,
                instruction: 0,
            },
            span: load.span,
        },
    )
}

fn is_exact_load(kind: &MirRvalueKind, storage: StorageId) -> bool {
    matches!(kind, MirRvalueKind::Load(place) if *place == MirPlace::base(storage))
}

fn storage_write_sites(
    definition: MirDefinitionRef<'_>,
    storage: StorageId,
) -> Vec<CheckedIntegerInstructionSite> {
    definition
        .body()
        .blocks
        .iter()
        .flat_map(|block| {
            block
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
                        block: block.id,
                        instruction,
                    })
                })
        })
        .collect()
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

fn invalid_value(
    definition: MirDefinitionRef<'_>,
    value: ValueId,
    site: CheckedIntegerInstructionSite,
) -> MirRewriteError {
    invalid_reference(
        definition,
        MirLocalIdentity::Value(value),
        MirLocalIdentitySite::Instruction {
            block: site.block.index(),
            instruction: site.instruction,
        },
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
#[path = "checked_integer_protocol/tests.rs"]
mod tests;
