//! Exact instruction and terminator shapes for checked-integer protocols.

use crate::{
    mir::{
        BlockId, MirBasicBlock, MirDefinitionRef, MirInstruction, MirPlace, MirRvalueKind,
        MirTerminator, MirType, StorageId,
    },
    source::Span,
};

use super::{CheckedIntegerInstructionSite, CheckedIntegerProtocolCheck, CheckedIntegerValueSite};

#[derive(Clone, Copy)]
pub(super) struct SuccessShape {
    pub(super) join_block: BlockId,
    pub(super) operand_loads: [CheckedIntegerValueSite; 2],
    pub(super) result_assignment: CheckedIntegerValueSite,
    pub(super) result_store: CheckedIntegerInstructionSite,
    pub(super) result_store_span: Span,
    pub(super) success_edge_span: Span,
}

pub(super) fn success_shape(
    block: &MirBasicBlock,
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

pub(in crate::passes::pipeline::optimizations) fn storage_write_sites(
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

pub(super) fn checked_terminator(
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
        MirTerminator::Return { .. }
        | MirTerminator::ReturnShared { .. }
        | MirTerminator::ReturnOptionalShared { .. }
        | MirTerminator::Panic { .. }
        | MirTerminator::Goto { .. }
        | MirTerminator::Branch { .. }
        | MirTerminator::PrimitiveCastRangeCheck { .. }
        | MirTerminator::CheckedCast { .. }
        | MirTerminator::SharedCast { .. }
        | MirTerminator::OptionalUnwrap { .. }
        | MirTerminator::OptionalSharedUnwrap { .. }
        | MirTerminator::BeginOptionalView { .. }
        | MirTerminator::BeginOptionalBoxView { .. }
        | MirTerminator::CheckOptionalMutation { .. }
        | MirTerminator::ArrayPositionCheck { .. }
        | MirTerminator::ArrayOperationCheck { .. }
        | MirTerminator::ArrayLoop { .. }
        | MirTerminator::Terminate { .. } => None,
    }
}

pub(super) fn exact_first_load(
    block: &MirBasicBlock,
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
