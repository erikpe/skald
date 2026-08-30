//! Verified-only final-MIR pipeline inspection.

use std::fmt;

use super::super::VerifiedFinalMirProgram;

/// Stable identity of one final-MIR pipeline inspection checkpoint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirPipelineCheckpointLabel {
    Input,
    After {
        position: usize,
        pass_name: &'static str,
        occurrence: usize,
    },
    Final,
}

impl fmt::Display for MirPipelineCheckpointLabel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Input => formatter.write_str("input"),
            Self::After {
                position,
                pass_name,
                occurrence,
            } => write!(formatter, "after-{position}-{pass_name}-{occurrence}"),
            Self::Final => formatter.write_str("final"),
        }
    }
}

/// One immutable verified final-MIR checkpoint.
///
/// The verified product is borrowed only for the callback. Inspection cannot
/// mutate it or retain it beyond the checkpoint invocation:
///
/// ```compile_fail
/// use skald_compiler::passes::MirPipelineCheckpoint;
///
/// fn mutate(checkpoint: MirPipelineCheckpoint<'_>) {
///     checkpoint.verified().program().definitions.clear();
/// }
/// ```
#[derive(Clone, Copy, Debug)]
pub struct MirPipelineCheckpoint<'a> {
    label: MirPipelineCheckpointLabel,
    verified: &'a VerifiedFinalMirProgram,
}

impl<'a> MirPipelineCheckpoint<'a> {
    pub const fn label(self) -> MirPipelineCheckpointLabel {
        self.label
    }

    pub const fn verified(self) -> &'a VerifiedFinalMirProgram {
        self.verified
    }

    pub(super) const fn new(
        label: MirPipelineCheckpointLabel,
        verified: &'a VerifiedFinalMirProgram,
    ) -> Self {
        Self { label, verified }
    }
}

/// Request-local consumer of borrowed verified final-MIR checkpoints.
///
/// The callback is deliberately independent of compilation requests and
/// operational report observers. Implementations may render [`crate::mir::dump_mir`]
/// or collect in-memory facts without receiving mutation, reporter, target, or
/// filesystem capabilities from the pipeline.
pub trait MirPipelineInspector {
    fn inspect(&mut self, checkpoint: MirPipelineCheckpoint<'_>);
}

impl<F> MirPipelineInspector for F
where
    F: for<'a> FnMut(MirPipelineCheckpoint<'a>),
{
    fn inspect(&mut self, checkpoint: MirPipelineCheckpoint<'_>) {
        self(checkpoint);
    }
}
