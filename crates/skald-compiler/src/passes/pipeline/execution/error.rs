use std::fmt;

use crate::mir::{rewrite::MirRewriteError, MirVerificationErrors};

use super::super::{MirPassOccurrence, MirPassStage};
use super::model::MirPassExecutionError;

/// Failure class owned by the stage-aware MIR pipeline.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirPipelineFailureStage {
    InputVerification,
    ProofNormalization,
    PassExecution,
    StructuralRewrite,
    OutputVerification,
}

/// Structured failure from verification, mandatory normalization, or one
/// selected pass.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirPipelineError {
    kind: MirPipelineErrorKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum MirPipelineErrorKind {
    InputVerification(Box<MirVerificationErrors>),
    PassExecution {
        occurrence: MirPassOccurrence,
        error: MirPassExecutionError,
    },
    StructuralRewrite {
        occurrence: MirPassOccurrence,
        error: Box<MirRewriteError>,
    },
    OutputVerification {
        occurrence: MirPassOccurrence,
        errors: Box<MirVerificationErrors>,
    },
    ProofNormalization(Box<MirVerificationErrors>),
}

impl MirPipelineError {
    pub const fn stage(&self) -> MirPipelineFailureStage {
        match self.kind {
            MirPipelineErrorKind::InputVerification(_) => {
                MirPipelineFailureStage::InputVerification
            }
            MirPipelineErrorKind::PassExecution { .. } => MirPipelineFailureStage::PassExecution,
            MirPipelineErrorKind::StructuralRewrite { .. } => {
                MirPipelineFailureStage::StructuralRewrite
            }
            MirPipelineErrorKind::OutputVerification { .. } => {
                MirPipelineFailureStage::OutputVerification
            }
            MirPipelineErrorKind::ProofNormalization(_) => {
                MirPipelineFailureStage::ProofNormalization
            }
        }
    }

    /// Zero-based schedule position for a pass-attributed failure.
    pub const fn pass_position(&self) -> Option<usize> {
        match self.occurrence() {
            Some(occurrence) => Some(occurrence.position()),
            None => None,
        }
    }

    /// Stable pass name for a pass-attributed failure.
    pub const fn pass_name(&self) -> Option<&'static str> {
        match self.occurrence() {
            Some(occurrence) => Some(occurrence.name()),
            None => None,
        }
    }

    /// MIR contract consumed by a pass-attributed failure.
    pub const fn pass_stage(&self) -> Option<MirPassStage> {
        match self.occurrence() {
            Some(occurrence) => Some(occurrence.stage()),
            None => None,
        }
    }

    /// Zero-based occurrence number for this pass identity.
    pub const fn pass_occurrence(&self) -> Option<usize> {
        match self.occurrence() {
            Some(occurrence) => Some(occurrence.occurrence()),
            None => None,
        }
    }

    pub(super) fn input_verification(errors: MirVerificationErrors) -> Self {
        Self {
            kind: MirPipelineErrorKind::InputVerification(Box::new(errors)),
        }
    }

    pub(super) fn pass_execution(
        occurrence: MirPassOccurrence,
        error: MirPassExecutionError,
    ) -> Self {
        Self {
            kind: MirPipelineErrorKind::PassExecution { occurrence, error },
        }
    }

    pub(super) fn structural_rewrite(
        occurrence: MirPassOccurrence,
        error: MirRewriteError,
    ) -> Self {
        Self {
            kind: MirPipelineErrorKind::StructuralRewrite {
                occurrence,
                error: Box::new(error),
            },
        }
    }

    pub(super) fn output_verification(
        occurrence: MirPassOccurrence,
        errors: MirVerificationErrors,
    ) -> Self {
        Self {
            kind: MirPipelineErrorKind::OutputVerification {
                occurrence,
                errors: Box::new(errors),
            },
        }
    }

    pub(super) fn proof_normalization(errors: MirVerificationErrors) -> Self {
        Self {
            kind: MirPipelineErrorKind::ProofNormalization(Box::new(errors)),
        }
    }

    const fn occurrence(&self) -> Option<MirPassOccurrence> {
        match &self.kind {
            MirPipelineErrorKind::InputVerification(_) => None,
            MirPipelineErrorKind::ProofNormalization(_) => None,
            MirPipelineErrorKind::PassExecution { occurrence, .. }
            | MirPipelineErrorKind::StructuralRewrite { occurrence, .. }
            | MirPipelineErrorKind::OutputVerification { occurrence, .. } => Some(*occurrence),
        }
    }
}

impl fmt::Display for MirPipelineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            MirPipelineErrorKind::InputVerification(errors) => {
                write!(
                    formatter,
                    "proof-rich MIR pipeline input verification failed: {errors}"
                )
            }
            MirPipelineErrorKind::PassExecution { occurrence, error } => {
                write_occurrence(formatter, *occurrence)?;
                write!(formatter, " execution failed: {error}")
            }
            MirPipelineErrorKind::StructuralRewrite { occurrence, error } => {
                write_occurrence(formatter, *occurrence)?;
                write!(formatter, " structural rewrite failed: {error}")
            }
            MirPipelineErrorKind::OutputVerification { occurrence, errors } => {
                write_occurrence(formatter, *occurrence)?;
                write!(formatter, " output verification failed: {errors}")
            }
            MirPipelineErrorKind::ProofNormalization(errors) => {
                write!(formatter, "proof-provenance normalization failed: {errors}")
            }
        }
    }
}

impl std::error::Error for MirPipelineError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self.kind {
            MirPipelineErrorKind::InputVerification(errors)
            | MirPipelineErrorKind::ProofNormalization(errors)
            | MirPipelineErrorKind::OutputVerification { errors, .. } => Some(errors.as_ref()),
            MirPipelineErrorKind::PassExecution { error, .. } => Some(error),
            MirPipelineErrorKind::StructuralRewrite { error, .. } => Some(error.as_ref()),
        }
    }
}

fn write_occurrence(
    formatter: &mut fmt::Formatter<'_>,
    occurrence: MirPassOccurrence,
) -> fmt::Result {
    write!(
        formatter,
        "{} MIR pass `{}` ({}, schedule position {}, occurrence {})",
        occurrence.stage(),
        occurrence.name(),
        occurrence.identity(),
        occurrence.position(),
        occurrence.occurrence()
    )
}
