//! Stage-aware verified MIR pipeline inspection.

use std::fmt;

use super::super::{MirPassStage, VerifiedFinalMirProgram, VerifiedProofMirProgram};
use crate::passes::reachability::dump_reachability;

/// Stable identity of one MIR pipeline inspection checkpoint.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirPipelineCheckpointLabel {
    ProofRichInput,
    AfterProofRichPass {
        position: usize,
        pass_name: &'static str,
        occurrence: usize,
    },
    AfterProofNormalization,
    AfterFinalPass {
        position: usize,
        pass_name: &'static str,
        occurrence: usize,
    },
    Final,
}

impl fmt::Display for MirPipelineCheckpointLabel {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProofRichInput => formatter.write_str("proof-rich-input"),
            Self::AfterProofRichPass {
                position,
                pass_name,
                occurrence,
            } => write!(
                formatter,
                "after-proof-rich-{position}-{pass_name}-{occurrence}"
            ),
            Self::AfterProofNormalization => formatter.write_str("after-proof-normalization"),
            Self::AfterFinalPass {
                position,
                pass_name,
                occurrence,
            } => write!(formatter, "after-final-{position}-{pass_name}-{occurrence}"),
            Self::Final => formatter.write_str("final"),
        }
    }
}

/// Closed borrowed view of one verified MIR pipeline checkpoint.
///
/// Pattern matching makes the applicable seal explicit. A proof-rich product
/// cannot be mistaken for normalized final MIR, and neither variant grants
/// mutation or ownership of the seal.
#[derive(Clone, Copy, Debug)]
pub enum MirPipelineCheckpoint<'a> {
    ProofRich(MirProofPipelineCheckpoint<'a>),
    Final(MirFinalPipelineCheckpoint<'a>),
}

impl MirPipelineCheckpoint<'_> {
    pub const fn label(self) -> MirPipelineCheckpointLabel {
        match self {
            Self::ProofRich(checkpoint) => checkpoint.label(),
            Self::Final(checkpoint) => checkpoint.label(),
        }
    }

    pub const fn stage(self) -> MirPassStage {
        match self {
            Self::ProofRich(_) => MirPassStage::ProofRich,
            Self::Final(_) => MirPassStage::Final,
        }
    }
}

/// One immutable verified proof-rich MIR checkpoint.
///
/// The product is borrowed only for the callback. Inspection cannot mutate it
/// or retain it beyond the checkpoint invocation:
///
/// ```compile_fail
/// use skald_compiler::passes::MirProofPipelineCheckpoint;
///
/// fn mutate(checkpoint: MirProofPipelineCheckpoint<'_>) {
///     checkpoint.verified().program().definitions.clear();
/// }
/// ```
#[derive(Clone, Copy, Debug)]
pub struct MirProofPipelineCheckpoint<'a> {
    label: MirPipelineCheckpointLabel,
    verified: &'a VerifiedProofMirProgram,
}

impl<'a> MirProofPipelineCheckpoint<'a> {
    pub const fn label(self) -> MirPipelineCheckpointLabel {
        self.label
    }

    pub const fn verified(self) -> &'a VerifiedProofMirProgram {
        self.verified
    }

    pub(super) const fn new(
        label: MirPipelineCheckpointLabel,
        verified: &'a VerifiedProofMirProgram,
    ) -> Self {
        Self { label, verified }
    }
}

/// One immutable normalized final-MIR checkpoint.
///
/// Final checkpoints alone expose the reachability facts sealed with the
/// exact normalized program. Rendering remains opt-in and does not create a
/// report event.
///
/// ```compile_fail
/// use skald_compiler::passes::MirFinalPipelineCheckpoint;
///
/// fn mutate(checkpoint: MirFinalPipelineCheckpoint<'_>) {
///     checkpoint.verified().program().definitions.clear();
/// }
/// ```
#[derive(Clone, Copy, Debug)]
pub struct MirFinalPipelineCheckpoint<'a> {
    label: MirPipelineCheckpointLabel,
    verified: &'a VerifiedFinalMirProgram,
}

impl<'a> MirFinalPipelineCheckpoint<'a> {
    pub const fn label(self) -> MirPipelineCheckpointLabel {
        self.label
    }

    pub const fn verified(self) -> &'a VerifiedFinalMirProgram {
        self.verified
    }

    /// Renders deterministic reachability facts bound to this final seal.
    pub fn reachability_dump(self) -> String {
        dump_reachability(self.verified.reachability())
    }

    pub(super) const fn new(
        label: MirPipelineCheckpointLabel,
        verified: &'a VerifiedFinalMirProgram,
    ) -> Self {
        Self { label, verified }
    }
}

/// Request-local consumer of borrowed verified MIR checkpoints.
///
/// The callback is deliberately independent of compilation requests and
/// operational report observers. Implementations may render
/// [`crate::mir::dump_mir`] or collect in-memory facts without receiving
/// mutation, reporter, target, or filesystem capabilities from the pipeline.
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
