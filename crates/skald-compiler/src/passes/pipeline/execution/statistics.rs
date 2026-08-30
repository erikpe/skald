use crate::mir::rewrite::{MirProgramRewriteResult, MirRewriteChangeSummary};

use super::{model::MirPassData, MirPipelineError};
use crate::passes::pipeline::VerifiedFinalMirProgram;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct MirPipelineStatistics {
    verification_executions: u64,
    pass_executions: u64,
    rewritten_callables: u64,
    changed_callables: u64,
    rewrite_changes: MirRewriteChangeSummary,
}

impl MirPipelineStatistics {
    pub(crate) const fn verification_executions(self) -> u64 {
        self.verification_executions
    }

    pub(crate) const fn pass_executions(self) -> u64 {
        self.pass_executions
    }

    pub(crate) const fn rewritten_callables(self) -> u64 {
        self.rewritten_callables
    }

    #[allow(dead_code)]
    pub(crate) const fn changed_callables(self) -> u64 {
        self.changed_callables
    }

    pub(crate) const fn rewrite_changes(self) -> MirRewriteChangeSummary {
        self.rewrite_changes
    }

    pub(super) fn record_verification(&mut self) {
        self.verification_executions = self.verification_executions.saturating_add(1);
    }

    pub(super) fn record_pass_execution(&mut self) {
        self.pass_executions = self.pass_executions.saturating_add(1);
    }

    pub(super) fn record_rewrite(&mut self, rewrite: &MirProgramRewriteResult, data: MirPassData) {
        self.rewritten_callables = self
            .rewritten_callables
            .saturating_add(u64::try_from(rewrite.callables.len()).unwrap_or(u64::MAX));
        self.changed_callables = self
            .changed_callables
            .saturating_add(u64::try_from(data.changed_callables()).unwrap_or(u64::MAX));
        for callable in &rewrite.callables {
            self.rewrite_changes.accumulate(callable.changes);
        }
    }
}

pub(crate) struct MeasuredMirPipeline {
    pub(crate) result: Result<VerifiedFinalMirProgram, MirPipelineError>,
    pub(crate) statistics: MirPipelineStatistics,
}
