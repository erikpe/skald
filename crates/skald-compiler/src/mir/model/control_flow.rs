//! MIR basic blocks and explicit control-flow termination.

use crate::source::Span;

use super::{
    ids::{BlockId, ValueId},
    instruction::{MirInstruction, MirNarrowedAliasBinding},
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
    /// Performs a metadata check and establishes `binding` only on success.
    CheckedNarrow {
        binding: MirNarrowedAliasBinding,
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
    NarrowingFailure,
}

impl MirTerminator {
    pub const fn span(&self) -> Span {
        match self {
            Self::Return { span, .. }
            | Self::Goto { span, .. }
            | Self::Branch { span, .. }
            | Self::CheckedNarrow { span, .. }
            | Self::Terminate { span, .. } => *span,
        }
    }

    /// Returns outgoing control-flow targets in semantic order. For a branch,
    /// the true edge always precedes the false edge.
    pub fn successors(&self) -> impl Iterator<Item = BlockId> {
        let targets = match self {
            Self::Return { .. } => [None, None],
            Self::Goto { target, .. } => [Some(*target), None],
            Self::Branch {
                true_target,
                false_target,
                ..
            } => [Some(*true_target), Some(*false_target)],
            Self::CheckedNarrow {
                success_target,
                failure_target,
                ..
            } => [Some(*success_target), Some(*failure_target)],
            Self::Terminate { .. } => [None, None],
        };
        targets.into_iter().flatten()
    }
}
