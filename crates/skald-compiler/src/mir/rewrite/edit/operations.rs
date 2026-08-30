//! Supported structural operations over sparse callable edit state.

use std::convert::Infallible;

use super::{super::error::MirRewriteError, MirCallableEdit};
use crate::mir::{BlockId, MirInstruction, MirTerminator, StorageId, ValueId};

use super::super::{
    map::{map_instruction, map_logical_expression, map_path_condition_metadata, map_terminator},
    MirLocalIdentityMapper, MirLocalIdentitySite,
};

impl MirCallableEdit {
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
        self.map_live_references(&mut mapper);
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
        self.map_live_references(&mut mapper);
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

    fn map_live_references(
        &mut self,
        mapper: &mut impl MirLocalIdentityMapper<Error = Infallible>,
    ) {
        for block in self.blocks.live_entries_mut() {
            for (instruction, entry) in block.instructions.iter_mut().enumerate() {
                infallible(map_instruction(
                    entry,
                    mapper,
                    MirLocalIdentitySite::Instruction {
                        block: block.id.index(),
                        instruction,
                    },
                ));
            }
            if let Some(terminator) = &mut block.terminator {
                infallible(map_terminator(
                    terminator,
                    mapper,
                    MirLocalIdentitySite::Terminator(block.id.index()),
                ));
            }
        }
        for condition in self.path_conditions.live_entries_mut() {
            infallible(map_path_condition_metadata(
                condition,
                mapper,
                MirLocalIdentitySite::PathCondition(condition.id.index()),
            ));
        }
        let logical_order = self.logical_expressions.order().to_vec();
        for index in logical_order {
            let expression = self
                .logical_expressions
                .get_mut(index)
                .expect("live logical order was established by the edit transaction");
            infallible(map_logical_expression(
                expression,
                mapper,
                MirLocalIdentitySite::LogicalExpression(index.index()),
            ));
        }
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
