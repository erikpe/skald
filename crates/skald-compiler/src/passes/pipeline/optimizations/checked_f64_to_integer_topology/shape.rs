//! Exact instruction and terminator shape for a checked floating cast.

use crate::{
    mir::{
        BlockId, MirBasicBlock, MirInstruction, MirPlace, MirPrimitiveCastRangeCheck,
        MirRvalueKind, MirTerminator,
    },
    source::Span,
};

use super::super::checked_scalar_topology::{
    is_exact_load, CheckedScalarInstructionSite, CheckedScalarValueSite,
};

#[derive(Clone, Copy)]
pub(super) struct SuccessShape {
    pub(super) join_block: BlockId,
    pub(super) source_load: CheckedScalarValueSite,
    pub(super) result_assignment: CheckedScalarValueSite,
    pub(super) result_store: CheckedScalarInstructionSite,
    pub(super) result_store_span: Span,
    pub(super) success_edge_span: Span,
}

pub(super) fn success_shape(
    block: &MirBasicBlock,
    check: MirPrimitiveCastRangeCheck,
) -> Option<SuccessShape> {
    let [MirInstruction::Assign(source), MirInstruction::Assign(result), MirInstruction::Store(store)] =
        block.instructions.as_slice()
    else {
        return None;
    };
    let result_type = check.relation.result_type();
    if !is_exact_load(&source.rvalue.kind, check.source)
        || source.rvalue.ty != crate::mir::MirType::F64
        || store.destination != MirPlace::base(check.result)
        || store.value != result.result
        || store.authorization.is_some()
        || store.final_authorization.is_some()
        || result.rvalue.ty != result_type
        || !matches!(
            result.rvalue.kind,
            MirRvalueKind::CheckedF64ToInteger { relation, operand }
                if relation == check.relation && operand == source.result
        )
    {
        return None;
    }
    let Some(MirTerminator::Goto { target, span }) = block.terminator else {
        return None;
    };
    Some(SuccessShape {
        join_block: target,
        source_load: CheckedScalarValueSite {
            value: source.result,
            site: CheckedScalarInstructionSite {
                block: block.id,
                instruction: 0,
            },
            span: source.span,
        },
        result_assignment: CheckedScalarValueSite {
            value: result.result,
            site: CheckedScalarInstructionSite {
                block: block.id,
                instruction: 1,
            },
            span: result.span,
        },
        result_store: CheckedScalarInstructionSite {
            block: block.id,
            instruction: 2,
        },
        result_store_span: store.span,
        success_edge_span: span,
    })
}

pub(super) fn checked_terminator(
    terminator: Option<&MirTerminator>,
) -> Option<(MirPrimitiveCastRangeCheck, BlockId, BlockId, Span)> {
    match terminator? {
        MirTerminator::PrimitiveCastRangeCheck {
            check,
            success_target,
            failure_target,
            span,
        } => Some((*check, *success_target, *failure_target, *span)),
        MirTerminator::Return { .. }
        | MirTerminator::ReturnShared { .. }
        | MirTerminator::ReturnOptionalShared { .. }
        | MirTerminator::Panic { .. }
        | MirTerminator::Goto { .. }
        | MirTerminator::Branch { .. }
        | MirTerminator::IntegerDivisorCheck { .. }
        | MirTerminator::ShiftCountCheck { .. }
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
