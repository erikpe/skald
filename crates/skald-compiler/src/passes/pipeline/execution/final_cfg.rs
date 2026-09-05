//! Narrow sparse-edit surface for normalized executable CFG rewrites.

mod canonicalization;

use crate::mir::{
    rewrite::{MirCallableEdit, MirFinalCfgFacts, MirRewriteError},
    MirStorage,
};

/// Exact storage declarations protected by the normalized CFG capability.
///
/// Current final-stage edits may remove whole unreachable blocks or move a
/// complete successor body during merging. They have no authority to create,
/// delete, or reclassify storage declarations. Keeping this guard at the
/// capability boundary makes that restriction fail closed if the wrapper is
/// extended later.
pub(super) struct MirFinalCfgStorageInvariant {
    declarations: Vec<MirStorage>,
}

impl MirFinalCfgStorageInvariant {
    pub(super) fn capture(edit: &MirCallableEdit) -> Result<Self, MirRewriteError> {
        let declarations = edit
            .storage_ids()
            .map(|storage| edit.storage(storage).cloned())
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self { declarations })
    }

    pub(super) fn verify(self, edit: &MirCallableEdit) -> Result<(), MirRewriteError> {
        let current = edit
            .storage_ids()
            .map(|storage| edit.storage(storage).cloned())
            .collect::<Result<Vec<_>, _>>()?;
        if current != self.declarations {
            return Err(MirRewriteError::UnsupportedFinalCfgStorageMutation {
                callable: edit.callable(),
            });
        }
        Ok(())
    }
}

/// Final-stage access to reviewed executable-CFG compound operations.
///
/// The wrapper deliberately exposes neither raw mutable MIR nor storage,
/// instruction, terminator, proof-record, or lifecycle mutation.
/// [`MirFinalPassCapability`](super::model::MirFinalPassCapability) additionally
/// rejects any storage-declaration change before dense commit, so adding a
/// storage-editing operation requires replacing this fail-closed contract
/// deliberately.
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
        self.require_current_facts(expected)?;

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

    fn require_current_facts(
        &self,
        expected: &MirFinalCfgFacts,
    ) -> Result<MirFinalCfgFacts, MirRewriteError> {
        let current = self.edit.final_cfg_facts()?;
        if current != *expected {
            return Err(MirRewriteError::StaleCallableSnapshot {
                callable: self.edit.callable(),
                subject: "normalized CFG facts",
            });
        }
        Ok(current)
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
