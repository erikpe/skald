//! Guarded compound edits authorized by normalized CFG candidate analysis.

use crate::mir::rewrite::{
    analyze_basic_block_merging, analyze_empty_block_forwarding, MirBasicBlockMergeCandidate,
    MirEmptyBlockForwardingPlan, MirFinalCfgFacts, MirRewriteError,
};

use super::MirFinalCfgEdit;

impl MirFinalCfgEdit<'_> {
    /// Applies one complete forwarding plan authorized by an exact normalized
    /// CFG snapshot.
    pub(in crate::passes::pipeline) fn apply_empty_block_forwarding(
        &mut self,
        expected: &MirFinalCfgFacts,
        plan: &MirEmptyBlockForwardingPlan,
    ) -> Result<MirFinalCfgForwarding, MirRewriteError> {
        self.validate_forwarding_identities(plan)?;
        let current = self.require_current_facts(expected)?;
        let authorized = analyze_empty_block_forwarding(&current);
        if authorized.plan() != plan {
            return Err(MirRewriteError::StaleCallableSnapshot {
                callable: self.edit.callable(),
                subject: "empty-block forwarding plan",
            });
        }

        let mut redirected_edges = 0usize;
        for resolution in plan.resolutions() {
            redirected_edges = redirected_edges.saturating_add(
                self.edit
                    .redirect_edges(resolution.block(), resolution.target())?,
            );
        }
        for resolution in plan.resolutions() {
            self.edit.remove_block(resolution.block())?;
        }

        Ok(MirFinalCfgForwarding {
            removed_blocks: plan.resolutions().len(),
            redirected_edges,
        })
    }

    /// Merges one exact linear pair authorized by an exact normalized CFG
    /// snapshot.
    pub(in crate::passes::pipeline) fn merge_basic_blocks(
        &mut self,
        expected: &MirFinalCfgFacts,
        candidate: MirBasicBlockMergeCandidate,
    ) -> Result<MirFinalCfgMerge, MirRewriteError> {
        self.edit.block(candidate.predecessor())?;
        self.edit.block(candidate.successor())?;
        let current = self.require_current_facts(expected)?;
        if !analyze_basic_block_merging(&current)
            .candidates()
            .contains(&candidate)
        {
            return Err(MirRewriteError::StaleCallableSnapshot {
                callable: self.edit.callable(),
                subject: "basic-block merge candidate",
            });
        }

        let moved_instructions = self
            .edit
            .merge_goto_successor(candidate.predecessor(), candidate.successor())?;
        Ok(MirFinalCfgMerge { moved_instructions })
    }

    fn validate_forwarding_identities(
        &self,
        plan: &MirEmptyBlockForwardingPlan,
    ) -> Result<(), MirRewriteError> {
        for resolution in plan.resolutions() {
            self.edit.block(resolution.block())?;
            self.edit.block(resolution.target())?;
        }
        Ok(())
    }
}

/// Deterministic changes made by one complete forwarding plan.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::passes::pipeline) struct MirFinalCfgForwarding {
    removed_blocks: usize,
    redirected_edges: usize,
}

impl MirFinalCfgForwarding {
    pub(in crate::passes::pipeline) const fn removed_blocks(self) -> usize {
        self.removed_blocks
    }

    pub(in crate::passes::pipeline) const fn redirected_edges(self) -> usize {
        self.redirected_edges
    }
}

/// Deterministic changes made by one exact block merge.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::passes::pipeline) struct MirFinalCfgMerge {
    moved_instructions: usize,
}

impl MirFinalCfgMerge {
    pub(in crate::passes::pipeline) const fn moved_instructions(self) -> usize {
        self.moved_instructions
    }
}

#[cfg(test)]
mod tests;
