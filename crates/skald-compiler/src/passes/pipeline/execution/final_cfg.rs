//! Narrow sparse-edit surface for normalized executable CFG deletion.

use crate::mir::rewrite::{MirCallableEdit, MirFinalCfgFacts, MirRewriteError};

/// Final-stage access to reviewed unreachable-block deletion operations.
///
/// The wrapper deliberately exposes neither raw mutable MIR nor storage,
/// instruction, terminator, proof-record, or lifecycle mutation.
pub(in crate::passes::pipeline) struct MirFinalCfgEdit<'edit> {
    edit: &'edit mut MirCallableEdit,
}

impl<'edit> MirFinalCfgEdit<'edit> {
    pub(super) fn new(edit: &'edit mut MirCallableEdit) -> Self {
        Self { edit }
    }

    /// Captures roots, adjacency, reachability, and block-owned values under
    /// the normalized CFG contract.
    pub(in crate::passes::pipeline) fn facts(&self) -> Result<MirFinalCfgFacts, MirRewriteError> {
        self.edit.final_cfg_facts()
    }

    /// Deletes every block and transient value classified as unreachable in
    /// an exact, still-current normalized CFG snapshot.
    pub(in crate::passes::pipeline) fn remove_unreachable_blocks(
        &mut self,
        expected: &MirFinalCfgFacts,
    ) -> Result<MirFinalCfgRemoval, MirRewriteError> {
        let current = self.edit.final_cfg_facts()?;
        if current != *expected {
            return Err(MirRewriteError::StaleCallableSnapshot {
                callable: self.edit.callable(),
                subject: "normalized CFG facts",
            });
        }

        let removals = expected
            .unreachable()
            .iter()
            .map(|block| {
                let values = expected
                    .block(*block)
                    .expect("unreachable block belongs to its CFG snapshot")
                    .defined_values()
                    .to_vec();
                (*block, values)
            })
            .collect::<Vec<_>>();

        let mut removed_values = 0usize;
        for (_, values) in &removals {
            for value in values {
                self.edit.remove_value(*value)?;
                removed_values = removed_values.saturating_add(1);
            }
        }
        for (block, _) in &removals {
            self.edit.remove_block(*block)?;
        }

        Ok(MirFinalCfgRemoval {
            blocks: removals.len(),
            values: removed_values,
        })
    }
}

/// Deterministic entity counts from one normalized callable edit.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::passes::pipeline) struct MirFinalCfgRemoval {
    blocks: usize,
    values: usize,
}

impl MirFinalCfgRemoval {
    pub(in crate::passes::pipeline) const fn blocks(self) -> usize {
        self.blocks
    }

    pub(in crate::passes::pipeline) const fn values(self) -> usize {
        self.values
    }
}

#[cfg(test)]
mod tests;
