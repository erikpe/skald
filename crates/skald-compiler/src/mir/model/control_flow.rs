//! MIR basic blocks and explicit control-flow termination.

use crate::source::Span;

use super::{
    ids::{BlockId, ValueId},
    instruction::{MirCheckedViewBinding, MirInstruction},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirBody {
    pub entry: BlockId,
    pub blocks: Vec<MirBasicBlock>,
    pub path_conditions: Vec<super::path_condition::MirPathCondition>,
    pub logical_expressions: Vec<super::logical::MirLogicalExpression>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirBasicBlock {
    pub id: BlockId,
    pub instructions: Vec<MirInstruction>,
    /// `None` is representable while constructing MIR so the verifier can
    /// diagnose unfinished blocks. Successful lowering always sets it.
    pub terminator: Option<MirTerminator>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirTerminator {
    Return {
        value: Option<ValueId>,
        span: Span,
    },
    /// Returns one live shared owner and transfers it to the caller.
    ReturnShared {
        owner: super::ids::StorageId,
        span: Span,
    },
    ReturnOptionalShared {
        owner: super::ids::StorageId,
        span: Span,
    },
    /// Reports a language panic message and exits unsuccessfully.
    Panic {
        message: super::value::MirPlace,
        span: Span,
    },
    Goto {
        target: BlockId,
        span: Span,
    },
    Branch {
        condition: ValueId,
        true_target: BlockId,
        false_target: BlockId,
        span: Span,
    },
    /// Rejects a count at or above the selected left operand width before the
    /// success block performs its corresponding shift.
    ShiftCountCheck {
        check: super::shift::MirShiftCountCheck,
        success_target: BlockId,
        failure_target: BlockId,
        span: Span,
    },
    /// Rejects a zero divisor before the success block performs its matching
    /// quotient or remainder operation.
    IntegerDivisorCheck {
        check: super::integer_division::MirIntegerDivisorCheck,
        success_target: BlockId,
        failure_target: BlockId,
        span: Span,
    },
    /// Proves the finite, truncation-toward-zero, and target-range relation
    /// before the success block performs its matching conversion.
    PrimitiveCastRangeCheck {
        check: super::primitive::MirPrimitiveCastRangeCheck,
        success_target: BlockId,
        failure_target: BlockId,
        span: Span,
    },
    /// Performs a metadata check and establishes one full-expression cast
    /// carrier only on success.
    CheckedCast {
        binding: MirCheckedViewBinding,
        success_target: BlockId,
        failure_target: BlockId,
        span: Span,
    },
    SharedCast {
        cast: super::shared::MirSharedCast,
        success_target: BlockId,
        failure_target: BlockId,
        span: Span,
    },
    OptionalUnwrap {
        source: super::value::MirPlace,
        destination: super::ids::StorageId,
        success_target: BlockId,
        failure_target: BlockId,
        span: Span,
    },
    OptionalSharedUnwrap {
        unwrap: super::optional::MirOptionalSharedUnwrap,
        success_target: BlockId,
        failure_target: BlockId,
        span: Span,
    },
    BeginOptionalView {
        begin: super::optional::MirOptionalViewBegin,
        success_target: BlockId,
        absent_target: BlockId,
        overflow_target: BlockId,
        span: Span,
    },
    BeginOptionalBoxView {
        begin: super::optional::MirOptionalBoxViewBegin,
        success_target: BlockId,
        absent_target: BlockId,
        overflow_target: BlockId,
        span: Span,
    },
    CheckOptionalMutation {
        source: super::value::MirPlace,
        success_target: BlockId,
        failure_target: BlockId,
        span: Span,
    },
    ArrayPositionCheck {
        position: super::ids::StorageId,
        kind: super::array::MirArrayPositionKind,
        success_target: BlockId,
        failure_target: BlockId,
        span: Span,
    },
    /// Branches on the checked result produced by the final array operation
    /// in this block. The failure successor must terminate with the exact
    /// language-defined reason corresponding to `failure`.
    ArrayOperationCheck {
        failure: super::array::MirArrayFailure,
        success_target: BlockId,
        failure_target: BlockId,
        span: Span,
    },
    /// Generated counted loop used for array construction, copying,
    /// assignment, and destruction.
    ArrayLoop {
        backing: super::ids::StorageId,
        index: super::ids::StorageId,
        length: super::ids::StorageId,
        kind: super::array::MirArrayLoopKind,
        body_target: BlockId,
        complete_target: BlockId,
        span: Span,
    },
    /// An explicit language-defined abnormal exit.
    Terminate {
        reason: MirTerminationReason,
        span: Span,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirTerminationReason {
    ObjectCastFailure,
    OptionalAccessFailure,
    OptionalGuardOverflow,
    OptionalPinnedMutation,
    ArrayAllocationFailure,
    ArrayIndexOutOfBounds,
    ArrayInvalidSliceBounds,
    ArraySliceLengthMismatch,
    ShiftCountOutOfRange,
    IntegerDivisionByZero,
    IntegerRemainderByZero,
    PrimitiveCastOutOfRange,
}

impl MirTerminationReason {
    pub const ALL: [Self; 12] = [
        Self::ObjectCastFailure,
        Self::OptionalAccessFailure,
        Self::OptionalGuardOverflow,
        Self::OptionalPinnedMutation,
        Self::ArrayAllocationFailure,
        Self::ArrayIndexOutOfBounds,
        Self::ArrayInvalidSliceBounds,
        Self::ArraySliceLengthMismatch,
        Self::ShiftCountOutOfRange,
        Self::IntegerDivisionByZero,
        Self::IntegerRemainderByZero,
        Self::PrimitiveCastOutOfRange,
    ];

    pub const fn mnemonic(self) -> &'static str {
        match self {
            Self::ObjectCastFailure => "object-cast-failure",
            Self::OptionalAccessFailure => "optional-access-failure",
            Self::OptionalGuardOverflow => "optional-guard-overflow",
            Self::OptionalPinnedMutation => "optional-pinned-mutation",
            Self::ArrayAllocationFailure => "array-allocation-failure",
            Self::ArrayIndexOutOfBounds => "array-index-out-of-bounds",
            Self::ArrayInvalidSliceBounds => "array-invalid-slice-bounds",
            Self::ArraySliceLengthMismatch => "array-slice-length-mismatch",
            Self::ShiftCountOutOfRange => "shift-count-out-of-range",
            Self::IntegerDivisionByZero => "integer-division-by-zero",
            Self::IntegerRemainderByZero => "integer-remainder-by-zero",
            Self::PrimitiveCastOutOfRange => "primitive-cast-out-of-range",
        }
    }
}

impl MirTerminator {
    pub const fn span(&self) -> Span {
        match self {
            Self::Return { span, .. }
            | Self::ReturnShared { span, .. }
            | Self::ReturnOptionalShared { span, .. }
            | Self::Panic { span, .. }
            | Self::Goto { span, .. }
            | Self::Branch { span, .. }
            | Self::ShiftCountCheck { span, .. }
            | Self::IntegerDivisorCheck { span, .. }
            | Self::PrimitiveCastRangeCheck { span, .. }
            | Self::CheckedCast { span, .. }
            | Self::SharedCast { span, .. }
            | Self::OptionalUnwrap { span, .. }
            | Self::OptionalSharedUnwrap { span, .. }
            | Self::BeginOptionalView { span, .. }
            | Self::BeginOptionalBoxView { span, .. }
            | Self::CheckOptionalMutation { span, .. }
            | Self::ArrayPositionCheck { span, .. }
            | Self::ArrayOperationCheck { span, .. }
            | Self::ArrayLoop { span, .. }
            | Self::Terminate { span, .. } => *span,
        }
    }

    /// Returns outgoing control-flow targets in semantic order. For a branch,
    /// the true edge always precedes the false edge.
    pub fn successors(&self) -> impl Iterator<Item = BlockId> {
        let targets = match self {
            Self::Return { .. }
            | Self::ReturnShared { .. }
            | Self::ReturnOptionalShared { .. }
            | Self::Panic { .. } => [None, None, None],
            Self::Goto { target, .. } => [Some(*target), None, None],
            Self::Branch {
                true_target,
                false_target,
                ..
            } => [Some(*true_target), Some(*false_target), None],
            Self::ShiftCountCheck {
                success_target,
                failure_target,
                ..
            } => [Some(*success_target), Some(*failure_target), None],
            Self::IntegerDivisorCheck {
                success_target,
                failure_target,
                ..
            } => [Some(*success_target), Some(*failure_target), None],
            Self::PrimitiveCastRangeCheck {
                success_target,
                failure_target,
                ..
            } => [Some(*success_target), Some(*failure_target), None],
            Self::CheckedCast {
                success_target,
                failure_target,
                ..
            } => [Some(*success_target), Some(*failure_target), None],
            Self::SharedCast {
                success_target,
                failure_target,
                ..
            } => [Some(*success_target), Some(*failure_target), None],
            Self::OptionalUnwrap {
                success_target,
                failure_target,
                ..
            } => [Some(*success_target), Some(*failure_target), None],
            Self::OptionalSharedUnwrap {
                success_target,
                failure_target,
                ..
            } => [Some(*success_target), Some(*failure_target), None],
            Self::BeginOptionalView {
                success_target,
                absent_target,
                overflow_target,
                ..
            } => [
                Some(*success_target),
                Some(*absent_target),
                Some(*overflow_target),
            ],
            Self::BeginOptionalBoxView {
                success_target,
                absent_target,
                overflow_target,
                ..
            } => [
                Some(*success_target),
                Some(*absent_target),
                Some(*overflow_target),
            ],
            Self::CheckOptionalMutation {
                success_target,
                failure_target,
                ..
            } => [Some(*success_target), Some(*failure_target), None],
            Self::ArrayPositionCheck {
                success_target,
                failure_target,
                ..
            } => [Some(*success_target), Some(*failure_target), None],
            Self::ArrayOperationCheck {
                success_target,
                failure_target,
                ..
            } => [Some(*success_target), Some(*failure_target), None],
            Self::ArrayLoop {
                body_target,
                complete_target,
                ..
            } => [Some(*body_target), Some(*complete_target), None],
            Self::Terminate { .. } => [None, None, None],
        };
        targets.into_iter().flatten()
    }
}
