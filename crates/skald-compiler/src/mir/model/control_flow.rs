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
    BeginOptionalView {
        begin: super::optional::MirOptionalViewBegin,
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
}

impl MirTerminator {
    pub const fn span(&self) -> Span {
        match self {
            Self::Return { span, .. }
            | Self::ReturnShared { span, .. }
            | Self::Goto { span, .. }
            | Self::Branch { span, .. }
            | Self::CheckedCast { span, .. }
            | Self::SharedCast { span, .. }
            | Self::OptionalUnwrap { span, .. }
            | Self::BeginOptionalView { span, .. }
            | Self::CheckOptionalMutation { span, .. }
            | Self::Terminate { span, .. } => *span,
        }
    }

    /// Returns outgoing control-flow targets in semantic order. For a branch,
    /// the true edge always precedes the false edge.
    pub fn successors(&self) -> impl Iterator<Item = BlockId> {
        let targets = match self {
            Self::Return { .. } | Self::ReturnShared { .. } => [None, None, None],
            Self::Goto { target, .. } => [Some(*target), None, None],
            Self::Branch {
                true_target,
                false_target,
                ..
            } => [Some(*true_target), Some(*false_target), None],
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
            Self::CheckOptionalMutation {
                success_target,
                failure_target,
                ..
            } => [Some(*success_target), Some(*failure_target), None],
            Self::Terminate { .. } => [None, None, None],
        };
        targets.into_iter().flatten()
    }
}
