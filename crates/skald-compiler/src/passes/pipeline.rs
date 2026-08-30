//! Target-independent MIR pass registration, execution, and accounting.

use std::ops::Deref;

use crate::mir::rewrite::MirRewriteChangeSummary;
use crate::mir::{MirProgram, MirVerificationErrors};

use super::static_lifecycle;

// This owner is intentionally dormant until the first production MIR pass.
// Its unit tests exercise the complete invalidation and resealing path now.
#[allow(dead_code)]
mod rewrite;

#[cfg(test)]
pub(crate) use rewrite::run_transforming_mir_pipeline;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct MirPipelineStatistics {
    verification_executions: u64,
    pass_executions: u64,
    rewritten_callables: u64,
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

    pub(crate) const fn rewrite_changes(self) -> MirRewriteChangeSummary {
        self.rewrite_changes
    }

    fn record_verification(&mut self) {
        self.verification_executions = self.verification_executions.saturating_add(1);
    }

    #[allow(dead_code)]
    fn record_pass_execution(&mut self) {
        self.pass_executions = self.pass_executions.saturating_add(1);
    }

    #[allow(dead_code)]
    fn record_rewrite(&mut self, rewrite: &crate::mir::rewrite::MirProgramRewriteResult) {
        self.rewritten_callables = self
            .rewritten_callables
            .saturating_add(u64::try_from(rewrite.callables.len()).unwrap_or(u64::MAX));
        for callable in &rewrite.callables {
            self.rewrite_changes.accumulate(callable.changes);
        }
    }
}

pub(crate) struct MeasuredMirPipeline {
    pub(crate) result: Result<VerifiedFinalMirProgram, MirVerificationErrors>,
    pub(crate) statistics: MirPipelineStatistics,
}

/// Read-only final MIR that passed ordinary and lifecycle-realization checks.
///
/// The private representation is the backend trust token. Any future pass
/// that changes executable MIR must produce raw MIR and call [`verify_final_mir`]
/// before constructing backend input again.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedFinalMirProgram {
    program: MirProgram,
}

impl VerifiedFinalMirProgram {
    pub const fn program(&self) -> &MirProgram {
        &self.program
    }

    /// Invalidates the final-MIR seal for a target-independent transformation.
    ///
    /// Visibility is deliberately restricted to the pass owner. Rewriters and
    /// backends cannot extract raw MIR from a verified product themselves.
    #[allow(dead_code)]
    fn invalidate_for_transformation(self) -> MirProgram {
        self.program
    }
}

impl Deref for VerifiedFinalMirProgram {
    type Target = MirProgram;

    fn deref(&self) -> &Self::Target {
        self.program()
    }
}

/// Runs the target-independent MIR pass pipeline.
///
/// No transformations are currently registered, but this explicit boundary
/// keeps correctness independent of a backend-owned implicit pipeline.
/// Verification runs here after MIR construction. The returned sealed product
/// is the only MIR accepted by backend input.
pub fn run_mir_pipeline(
    program: MirProgram,
) -> Result<VerifiedFinalMirProgram, MirVerificationErrors> {
    run_mir_pipeline_measured(program).result
}

/// Seals final MIR after the central ordinary and lifecycle-realization check.
///
/// This is the invalidation target for future transformations that can change
/// static accesses, control-flow reachability, lifecycle operations, or
/// possible callees. Passes that affect any of those facts must return raw MIR
/// to this boundary before backend input can be constructed.
pub fn verify_final_mir(
    program: MirProgram,
) -> Result<VerifiedFinalMirProgram, MirVerificationErrors> {
    static_lifecycle::verify_synthesized_mir(&program)?;
    Ok(VerifiedFinalMirProgram { program })
}

/// Runs the pipeline while retaining its already-known execution counts.
///
/// A future transformation must return its transformed program together with
/// pass-owned statistics to this coordinator. The pipeline, rather than the
/// pass or driver, then records the execution and publishes those values. A
/// pass must not format or emit reporting text itself.
pub(crate) fn run_mir_pipeline_measured(program: MirProgram) -> MeasuredMirPipeline {
    let mut statistics = MirPipelineStatistics::default();
    statistics.record_verification();
    let result = verify_final_mir(program);
    MeasuredMirPipeline { result, statistics }
}

#[cfg(test)]
mod tests;
