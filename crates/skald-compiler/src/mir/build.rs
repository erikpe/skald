//! Deterministic helpers for constructing MIR control-flow graphs.

use std::fmt;

use crate::{identity::CallableId, source::Span};

use super::{BlockId, MirBasicBlock, MirBody, MirInstruction, MirTerminator};

/// A small stateful builder that keeps block allocation and termination
/// invariants in one place. Blocks are allocated in stable ID order; changing
/// the selected block never changes that order.
pub(super) struct MirBodyBuilder {
    callable: CallableId,
    entry: BlockId,
    blocks: Vec<MirBasicBlock>,
    current: BlockId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum MirBuildError {
    UnknownBlock(BlockId),
    BlockAlreadyTerminated(BlockId),
}

impl fmt::Display for MirBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownBlock(block) => write!(formatter, "MIR block {block} is not allocated"),
            Self::BlockAlreadyTerminated(block) => {
                write!(formatter, "MIR block {block} is already terminated")
            }
        }
    }
}

impl std::error::Error for MirBuildError {}

impl MirBodyBuilder {
    pub(super) fn new(callable: impl Into<CallableId>, entry_span: Span) -> Self {
        let callable = callable.into();
        let entry = BlockId::new(callable, 0);
        Self {
            callable,
            entry,
            blocks: vec![MirBasicBlock {
                id: entry,
                instructions: Vec::new(),
                terminator: None,
                span: entry_span,
            }],
            current: entry,
        }
    }

    #[cfg(test)]
    pub(super) const fn entry(&self) -> BlockId {
        self.entry
    }

    pub(super) const fn current(&self) -> BlockId {
        self.current
    }

    pub(super) fn allocate_block(&mut self, span: Span) -> BlockId {
        let id = BlockId::new(self.callable, self.blocks.len());
        self.blocks.push(MirBasicBlock {
            id,
            instructions: Vec::new(),
            terminator: None,
            span,
        });
        id
    }

    pub(super) fn select_block(&mut self, block: BlockId) -> Result<(), MirBuildError> {
        self.block(block)?;
        self.current = block;
        Ok(())
    }

    pub(super) fn is_current_terminated(&self) -> bool {
        self.current_block().terminator.is_some()
    }

    pub(super) fn push_instruction(
        &mut self,
        instruction: MirInstruction,
    ) -> Result<(), MirBuildError> {
        let block = self.current_block_mut();
        if block.terminator.is_some() {
            return Err(MirBuildError::BlockAlreadyTerminated(block.id));
        }
        block.instructions.push(instruction);
        Ok(())
    }

    pub(super) fn terminate(&mut self, terminator: MirTerminator) -> Result<(), MirBuildError> {
        let block = self.current_block_mut();
        if block.terminator.is_some() {
            return Err(MirBuildError::BlockAlreadyTerminated(block.id));
        }
        block.terminator = Some(terminator);
        Ok(())
    }

    pub(super) fn finish(self) -> MirBody {
        MirBody {
            entry: self.entry,
            blocks: self.blocks,
            path_conditions: Vec::new(),
        }
    }

    fn block(&self, block: BlockId) -> Result<&MirBasicBlock, MirBuildError> {
        (block.callable() == self.callable)
            .then(|| self.blocks.get(block.index()))
            .flatten()
            .filter(|candidate| candidate.id == block)
            .ok_or(MirBuildError::UnknownBlock(block))
    }

    fn current_block(&self) -> &MirBasicBlock {
        &self.blocks[self.current.index()]
    }

    fn current_block_mut(&mut self) -> &mut MirBasicBlock {
        &mut self.blocks[self.current.index()]
    }
}
