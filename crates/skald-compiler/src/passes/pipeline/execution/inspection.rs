//! Verified proof-rich MIR pipeline inspection.

use std::fmt;

use super::super::VerifiedProofMirProgram;
use crate::passes::reachability::dump_reachability;

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

/// One immutable verified proof-rich MIR checkpoint.
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
    verified: &'a VerifiedProofMirProgram,
}

impl<'a> MirPipelineCheckpoint<'a> {
    pub const fn label(self) -> MirPipelineCheckpointLabel {
        self.label
    }

    pub const fn verified(self) -> &'a VerifiedProofMirProgram {
        self.verified
    }

    /// Renders the deterministic reachability facts sealed with this exact
    /// checkpoint for focused compiler tools and tests.
    ///
    /// The dump is constructed only when requested by an inspector. It is not
    /// a report event or a command-line publication policy.
    pub fn reachability_dump(self) -> String {
        dump_reachability(self.verified.reachability())
    }

    pub(super) const fn new(
        label: MirPipelineCheckpointLabel,
        verified: &'a VerifiedProofMirProgram,
    ) -> Self {
        Self { label, verified }
    }
}

/// Request-local consumer of borrowed verified proof-rich MIR checkpoints.
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
