//! Narrow ownership boundary between proof-rich and normalized final MIR.

use crate::passes::pipeline::{
    normalization::{MirProofNormalizationStatistics, MirProofTransitionPlan},
    seal::MirProofTransitionError,
    VerifiedFinalMirProgram, VerifiedProofMirProgram,
};

use super::model::{MirPassData, MirPassFailure};

/// Pipeline-owned operation which consumes proof provenance and seals final
/// MIR. The injected form exists solely so boundary failures can be exercised
/// without exposing either raw product.
pub(in crate::passes::pipeline) type ProofNormalizationTransition = fn(
    VerifiedProofMirProgram,
    Option<MirProofTransitionPlan>,
) -> Result<
    (VerifiedFinalMirProgram, MirProofNormalizationStatistics),
    MirProofTransitionError,
>;

/// Capability held by the sole optional proof-transition occurrence.
///
/// A transition may inspect proof-rich MIR, but its only consuming operation
/// accepts the narrow optional normalization plan and returns a verified final
/// product. It cannot obtain raw mutable MIR or publish an intermediate seal.
pub(in crate::passes::pipeline) struct MirProofTransitionCapability {
    verified: VerifiedProofMirProgram,
    transition: ProofNormalizationTransition,
}

impl MirProofTransitionCapability {
    pub(super) fn with_transition(
        verified: VerifiedProofMirProgram,
        transition: ProofNormalizationTransition,
    ) -> Self {
        Self {
            verified,
            transition,
        }
    }

    pub(in crate::passes::pipeline) const fn verified(&self) -> &VerifiedProofMirProgram {
        &self.verified
    }

    pub(in crate::passes::pipeline) fn normalize(
        self,
        optional_plan: Option<MirProofTransitionPlan>,
        data: MirPassData,
    ) -> Result<MirProofTransitionOutcome, MirProofTransitionFailure> {
        let has_optional_plan = optional_plan.is_some();
        if !has_optional_plan && data.changed_callables() != 0 {
            return Err(MirPassFailure::execution(
                "a no-op proof transition reported changed callables",
            )
            .into());
        }
        if let Some(plan) = &optional_plan {
            if data.processed_callables() != plan.processed_callables()
                || data.changed_callables() != plan.changed_callable_count()
            {
                return Err(MirPassFailure::execution(
                    "proof-transition plan and callable accounting disagree",
                )
                .into());
            }
        }
        let (verified, normalization) = (self.transition)(self.verified, optional_plan)
            .map_err(MirProofTransitionFailure::boundary)?;
        Ok(MirProofTransitionOutcome {
            verified,
            normalization,
            data,
            changed: has_optional_plan,
        })
    }
}

/// Final-sealed result of one selected transition occurrence.
pub(in crate::passes::pipeline) struct MirProofTransitionOutcome {
    verified: VerifiedFinalMirProgram,
    normalization: MirProofNormalizationStatistics,
    data: MirPassData,
    changed: bool,
}

impl MirProofTransitionOutcome {
    pub(super) fn into_parts(
        self,
    ) -> (
        VerifiedFinalMirProgram,
        MirProofNormalizationStatistics,
        MirPassData,
        bool,
    ) {
        (self.verified, self.normalization, self.data, self.changed)
    }
}

/// Failure returned before a transition can publish a verified final product.
pub(in crate::passes::pipeline) struct MirProofTransitionFailure {
    kind: MirProofTransitionFailureKind,
}

pub(in crate::passes::pipeline) enum MirProofTransitionFailureKind {
    Pass(MirPassFailure),
    Boundary(MirProofTransitionError),
}

impl MirProofTransitionFailure {
    fn boundary(error: MirProofTransitionError) -> Self {
        Self {
            kind: MirProofTransitionFailureKind::Boundary(error),
        }
    }

    pub(super) fn into_kind(self) -> MirProofTransitionFailureKind {
        self.kind
    }
}

impl From<MirPassFailure> for MirProofTransitionFailure {
    fn from(error: MirPassFailure) -> Self {
        Self {
            kind: MirProofTransitionFailureKind::Pass(error),
        }
    }
}

pub(in crate::passes::pipeline) type MirProofTransitionTransform =
    fn(
        MirProofTransitionCapability,
    ) -> Result<MirProofTransitionOutcome, MirProofTransitionFailure>;
