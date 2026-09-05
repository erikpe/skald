//! Supported structural operations over sparse callable edit state.

use std::convert::Infallible;

use super::{super::error::MirRewriteError, MirCallableEdit};
use crate::mir::{BlockId, MirInstruction, MirStorageKind, MirTerminator, StorageId, ValueId};

use super::super::{
    map::{
        map_instruction, map_logical_expression, map_path_condition_metadata, map_terminator,
        observe_instruction, observe_logical_expression, observe_path_condition_metadata,
        observe_terminator,
    },
    MirLocalIdentityMapper, MirLocalIdentityObserver, MirLocalIdentitySite,
};

impl MirCallableEdit {
    /// Reclassifies one storage declaration after checking the analysis
    /// snapshot which authorized the edit.
    pub(crate) fn replace_storage_kind(
        &mut self,
        storage: StorageId,
        expected: MirStorageKind,
        replacement: MirStorageKind,
    ) -> Result<(), MirRewriteError> {
        let declaration = self.storage.get_mut(storage)?;
        if declaration.kind != expected {
            return Err(MirRewriteError::StorageKindMismatch {
                storage,
                expected,
                actual: declaration.kind,
            });
        }
        declaration.kind = replacement;
        Ok(())
    }

    /// Replaces one instruction after checking the exact analyzed snapshot.
    pub(crate) fn replace_instruction(
        &mut self,
        block: BlockId,
        index: usize,
        expected: &MirInstruction,
        replacement: MirInstruction,
    ) -> Result<(), MirRewriteError> {
        let instructions = &mut self.blocks.get_mut(block)?.instructions;
        let Some(instruction) = instructions.get_mut(index) else {
            return Err(MirRewriteError::StaleCallableSnapshot {
                callable: self.callable,
                subject: "instruction position",
            });
        };
        if instruction != expected {
            return Err(MirRewriteError::StaleCallableSnapshot {
                callable: self.callable,
                subject: "instruction",
            });
        }
        *instruction = replacement;
        Ok(())
    }

    /// Observes all block roots outside ordinary executable successor edges.
    ///
    /// Path and logical metadata use the same exhaustive structural walkers
    /// as commit. Callable-header attachments are snapshots supplied by the
    /// package which owns their authoritative representation.
    pub(in crate::mir::rewrite) fn observe_cfg_roots<O: MirLocalIdentityObserver>(
        &self,
        observer: &mut O,
    ) -> Result<(), O::Error> {
        observer.observe_block(MirLocalIdentitySite::BodyEntry, self.entry)?;
        for (site, block) in &self.attachment_blocks {
            observer.observe_block(*site, *block)?;
        }
        for condition in self.path_conditions.live_entries() {
            observe_path_condition_metadata(
                condition,
                observer,
                MirLocalIdentitySite::PathCondition(condition.id.index()),
            )?;
        }
        for index in self.logical_expressions.order() {
            let expression = self
                .logical_expressions
                .get(*index)
                .expect("live logical order was established by the edit transaction");
            observe_logical_expression(
                expression,
                observer,
                MirLocalIdentitySite::LogicalExpression(index.index()),
            )?;
        }
        Ok(())
    }

    /// Replaces one block's instruction list as a single functional edit.
    ///
    /// Instruction positions are deliberately exposed only through this
    /// borrowed snapshot. They are not identities and do not survive a later
    /// rewrite of the same block.
    pub(crate) fn rewrite_block_instructions(
        &mut self,
        block: BlockId,
        rewrite: impl FnOnce(&[MirInstruction]) -> Vec<MirInstruction>,
    ) -> Result<(), MirRewriteError> {
        let block = self.blocks.get_mut(block)?;
        block.instructions = rewrite(&block.instructions);
        Ok(())
    }

    /// Replaces one block's optional terminator without exposing mutable block
    /// storage.
    pub(crate) fn rewrite_block_terminator(
        &mut self,
        block: BlockId,
        rewrite: impl FnOnce(Option<&MirTerminator>) -> Option<MirTerminator>,
    ) -> Result<(), MirRewriteError> {
        let block = self.blocks.get_mut(block)?;
        block.terminator = rewrite(block.terminator.as_ref());
        Ok(())
    }

    /// Replaces one terminator after checking the exact analyzed snapshot.
    pub(crate) fn replace_terminator(
        &mut self,
        block: BlockId,
        expected: &MirTerminator,
        replacement: MirTerminator,
    ) -> Result<(), MirRewriteError> {
        let terminator = &mut self.blocks.get_mut(block)?.terminator;
        if terminator.as_ref() != Some(expected) {
            return Err(MirRewriteError::StaleCallableSnapshot {
                callable: self.callable,
                subject: "terminator",
            });
        }
        *terminator = Some(replacement);
        Ok(())
    }

    /// Replaces all uses of `from` with `to`, preserving value definitions.
    ///
    /// Both values must be live, callable-local, and have the same MIR type.
    /// The caller remains responsible for dominance and semantic equivalence,
    /// and must explicitly remove an obsolete declaration or definition.
    pub(crate) fn replace_value_uses(
        &mut self,
        from: ValueId,
        to: ValueId,
    ) -> Result<usize, MirRewriteError> {
        let from_type = self.value(from)?.ty;
        let to_type = self.value(to)?.ty;
        if from_type != to_type {
            return Err(MirRewriteError::ValueTypeMismatch {
                from,
                from_type,
                to,
                to_type,
            });
        }
        let mut mapper = ValueUseSubstitution {
            from,
            to,
            replacements: 0,
        };
        infallible(self.map_live_references(&mut mapper));
        Ok(mapper.replacements)
    }

    /// Replaces every callable-body reference to one storage with another.
    ///
    /// Storage declarations and callable header attachments are not changed.
    /// Both slots must be live, callable-local, and have the same MIR type.
    /// The caller must explicitly update liveness operations and proof
    /// metadata when those should be deleted rather than substituted.
    pub(crate) fn replace_storage_uses(
        &mut self,
        from: StorageId,
        to: StorageId,
    ) -> Result<usize, MirRewriteError> {
        let from_type = self.storage(from)?.ty;
        let to_type = self.storage(to)?.ty;
        if from_type != to_type {
            return Err(MirRewriteError::StorageTypeMismatch {
                from,
                from_type,
                to,
                to_type,
            });
        }
        let mut mapper = StorageUseSubstitution {
            from,
            to,
            replacements: 0,
        };
        infallible(self.map_live_references(&mut mapper));
        Ok(mapper.replacements)
    }

    /// Redirects executable successor edges targeting `from` to `to`.
    ///
    /// This operation does not change body entry, path-condition provenance,
    /// logical-expression provenance, or static-publication attachments.
    pub(crate) fn redirect_edges(
        &mut self,
        from: BlockId,
        to: BlockId,
    ) -> Result<usize, MirRewriteError> {
        self.block(from)?;
        self.block(to)?;
        let mut mapper = EdgeRedirect {
            from,
            to,
            replacements: 0,
        };
        for block in self.blocks.live_entries_mut() {
            let Some(terminator) = &mut block.terminator else {
                continue;
            };
            infallible(map_terminator(
                terminator,
                &mut mapper,
                MirLocalIdentitySite::Terminator(block.id.index()),
            ));
        }
        Ok(mapper.replacements)
    }

    /// Appends one exact goto successor's instructions, transfers its
    /// terminator, and removes the successor block.
    ///
    /// This structural primitive verifies only the local shape needed to move
    /// complete block contents without partial failure. Its caller owns the
    /// semantic proof that the edge is unique and neither endpoint is a
    /// permanent attachment.
    pub(crate) fn merge_goto_successor(
        &mut self,
        predecessor: BlockId,
        successor: BlockId,
    ) -> Result<usize, MirRewriteError> {
        if predecessor == successor {
            return Err(MirRewriteError::StaleCallableSnapshot {
                callable: self.callable,
                subject: "distinct goto successor",
            });
        }

        let predecessor_block = self.blocks.get(predecessor)?;
        if !matches!(
            predecessor_block.terminator,
            Some(MirTerminator::Goto { target, .. }) if target == successor
        ) {
            return Err(MirRewriteError::StaleCallableSnapshot {
                callable: self.callable,
                subject: "goto predecessor",
            });
        }
        let successor_block = self.blocks.get(successor)?;
        let successor_terminator = successor_block
            .terminator
            .clone()
            .ok_or(MirRewriteError::MissingBlockTerminator { block: successor })?;
        let successor_instructions = successor_block.instructions.clone();
        if !self.block_order.contains(successor) {
            return Err(MirRewriteError::MissingOrderIdentity {
                identity: super::super::MirLocalIdentity::Block(successor),
            });
        }

        let moved_instructions = successor_instructions.len();
        let predecessor_block = self
            .blocks
            .get_mut(predecessor)
            .expect("validated predecessor remains live during one compound edit");
        predecessor_block
            .instructions
            .extend(successor_instructions);
        predecessor_block.terminator = Some(successor_terminator);
        self.remove_block(successor)
            .expect("validated successor and block order remain live during one compound edit");
        Ok(moved_instructions)
    }

    pub(in crate::mir::rewrite) fn map_live_references<M: MirLocalIdentityMapper>(
        &mut self,
        mapper: &mut M,
    ) -> Result<(), M::Error> {
        for block in self.blocks.live_entries_mut() {
            for (instruction, entry) in block.instructions.iter_mut().enumerate() {
                map_instruction(
                    entry,
                    mapper,
                    MirLocalIdentitySite::Instruction {
                        block: block.id.index(),
                        instruction,
                    },
                )?;
            }
            if let Some(terminator) = &mut block.terminator {
                map_terminator(
                    terminator,
                    mapper,
                    MirLocalIdentitySite::Terminator(block.id.index()),
                )?;
            }
        }
        for condition in self.path_conditions.live_entries_mut() {
            map_path_condition_metadata(
                condition,
                mapper,
                MirLocalIdentitySite::PathCondition(condition.id.index()),
            )?;
        }
        let logical_order = self.logical_expressions.order().to_vec();
        for index in logical_order {
            let expression = self
                .logical_expressions
                .get_mut(index)
                .expect("live logical order was established by the edit transaction");
            map_logical_expression(
                expression,
                mapper,
                MirLocalIdentitySite::LogicalExpression(index.index()),
            )?;
        }
        Ok(())
    }

    /// Observes all identities in live executable and proof-bearing edit state.
    ///
    /// Declarations, callable attachments, body entry, and block declarations
    /// are intentionally excluded: this is the read-only counterpart of
    /// [`Self::map_live_references`] used by reference analyses.
    pub(in crate::mir::rewrite) fn observe_live_references<O: MirLocalIdentityObserver>(
        &self,
        observer: &mut O,
    ) -> Result<(), O::Error> {
        for block in self.blocks.live_entries() {
            for (instruction, entry) in block.instructions.iter().enumerate() {
                observe_instruction(
                    entry,
                    observer,
                    MirLocalIdentitySite::Instruction {
                        block: block.id.index(),
                        instruction,
                    },
                )?;
            }
            if let Some(terminator) = &block.terminator {
                observe_terminator(
                    terminator,
                    observer,
                    MirLocalIdentitySite::Terminator(block.id.index()),
                )?;
            }
        }
        for condition in self.path_conditions.live_entries() {
            observe_path_condition_metadata(
                condition,
                observer,
                MirLocalIdentitySite::PathCondition(condition.id.index()),
            )?;
        }
        for index in self.logical_expressions.order() {
            let expression = self
                .logical_expressions
                .get(*index)
                .expect("live logical order was established by the edit transaction");
            observe_logical_expression(
                expression,
                observer,
                MirLocalIdentitySite::LogicalExpression(index.index()),
            )?;
        }
        Ok(())
    }
}

struct ValueUseSubstitution {
    from: ValueId,
    to: ValueId,
    replacements: usize,
}

impl MirLocalIdentityMapper for ValueUseSubstitution {
    type Error = Infallible;

    fn map_value(
        &mut self,
        _site: MirLocalIdentitySite,
        identity: ValueId,
    ) -> Result<ValueId, Self::Error> {
        if identity == self.from && self.from != self.to {
            self.replacements += 1;
            Ok(self.to)
        } else {
            Ok(identity)
        }
    }

    fn map_value_definition(
        &mut self,
        _site: MirLocalIdentitySite,
        identity: ValueId,
    ) -> Result<ValueId, Self::Error> {
        Ok(identity)
    }
}

struct StorageUseSubstitution {
    from: StorageId,
    to: StorageId,
    replacements: usize,
}

impl MirLocalIdentityMapper for StorageUseSubstitution {
    type Error = Infallible;

    fn map_storage(
        &mut self,
        _site: MirLocalIdentitySite,
        identity: StorageId,
    ) -> Result<StorageId, Self::Error> {
        if identity == self.from && self.from != self.to {
            self.replacements += 1;
            Ok(self.to)
        } else {
            Ok(identity)
        }
    }
}

struct EdgeRedirect {
    from: BlockId,
    to: BlockId,
    replacements: usize,
}

impl MirLocalIdentityMapper for EdgeRedirect {
    type Error = Infallible;

    fn map_block(
        &mut self,
        _site: MirLocalIdentitySite,
        identity: BlockId,
    ) -> Result<BlockId, Self::Error> {
        if identity == self.from && self.from != self.to {
            self.replacements += 1;
            Ok(self.to)
        } else {
            Ok(identity)
        }
    }
}

fn infallible(result: Result<(), Infallible>) {
    match result {
        Ok(()) => {}
        Err(error) => match error {},
    }
}

#[cfg(test)]
mod tests;
