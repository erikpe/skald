use std::fmt;

use crate::{
    identity::CallableId,
    mir::rewrite::{rewrite_program, MirCallableEdit, MirProgramRewriteResult, MirRewriteError},
};

use super::super::VerifiedFinalMirProgram;

/// Deterministic internal failure reported by a pass outside dense commit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::passes::pipeline) struct MirPassExecutionError {
    message: String,
}

impl MirPassExecutionError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for MirPassExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.message.fmt(formatter)
    }
}

impl std::error::Error for MirPassExecutionError {}

/// Pass-owned data retained with an explicit unchanged or changed outcome.
///
/// Per-pass integer measurements are added by the reporting milestone. The
/// changed-callable count is carried now because dense commit deliberately
/// processes every executable callable, whether the pass edited it or not.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::passes::pipeline) struct MirPassData {
    changed_callables: usize,
}

impl MirPassData {
    #[allow(dead_code)]
    pub(in crate::passes::pipeline) const fn changed(changed_callables: usize) -> Self {
        Self { changed_callables }
    }

    pub(super) const fn changed_callables(self) -> usize {
        self.changed_callables
    }
}

/// Pipeline-owned pass capability.
///
/// A pass may borrow the verified input for analysis. It can invalidate that
/// seal only by consuming this capability into the atomic all-program rewrite
/// coordinator.
pub(in crate::passes::pipeline) struct MirPassCapability {
    verified: VerifiedFinalMirProgram,
}

impl MirPassCapability {
    pub(super) fn new(verified: VerifiedFinalMirProgram) -> Self {
        Self { verified }
    }

    #[allow(dead_code)]
    pub(in crate::passes::pipeline) const fn verified(&self) -> &VerifiedFinalMirProgram {
        &self.verified
    }

    #[allow(dead_code)]
    pub(in crate::passes::pipeline) fn unchanged(self) -> MirPassOutcome {
        MirPassOutcome::Unchanged {
            verified: self.verified,
            data: MirPassData::default(),
        }
    }

    #[allow(dead_code)]
    pub(in crate::passes::pipeline) fn rewrite(
        self,
        rewrite: impl FnMut(CallableId, &mut MirCallableEdit) -> Result<(), MirRewriteError>,
    ) -> Result<MirChangedProgram, MirPassFailure> {
        rewrite_program(self.verified.invalidate_for_transformation(), rewrite)
            .map(|rewrite| MirChangedProgram { rewrite })
            .map_err(MirPassFailure::Rewrite)
    }
}

/// Successful atomic dense commit awaiting pass-owned change accounting.
pub(in crate::passes::pipeline) struct MirChangedProgram {
    rewrite: MirProgramRewriteResult,
}

impl MirChangedProgram {
    #[allow(dead_code)]
    pub(in crate::passes::pipeline) fn finish(
        self,
        data: MirPassData,
    ) -> Result<MirPassOutcome, MirPassFailure> {
        if data.changed_callables() > self.rewrite.callables.len() {
            return Err(MirPassFailure::execution(format!(
                "pass reported {} changed callables after processing only {}",
                data.changed_callables(),
                self.rewrite.callables.len()
            )));
        }
        Ok(MirPassOutcome::Changed {
            rewrite: self.rewrite,
            data,
        })
    }
}

/// Explicit ownership result from one pass occurrence.
pub(in crate::passes::pipeline) enum MirPassOutcome {
    Unchanged {
        verified: VerifiedFinalMirProgram,
        data: MirPassData,
    },
    Changed {
        rewrite: MirProgramRewriteResult,
        data: MirPassData,
    },
}

/// Failure returned by a pass before the pipeline can verify an output.
pub(in crate::passes::pipeline) enum MirPassFailure {
    Execution(MirPassExecutionError),
    Rewrite(MirRewriteError),
}

impl MirPassFailure {
    #[allow(dead_code)]
    pub(in crate::passes::pipeline) fn execution(message: impl Into<String>) -> Self {
        Self::Execution(MirPassExecutionError::new(message))
    }
}

pub(in crate::passes::pipeline) type MirPassTransform =
    fn(MirPassCapability) -> Result<MirPassOutcome, MirPassFailure>;
