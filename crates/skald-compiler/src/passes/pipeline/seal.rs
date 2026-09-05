//! Proof-rich and normalized final-MIR trust products.

use std::{fmt, ops::Deref};

use crate::mir::{MirProgram, MirVerificationErrors};

use super::{
    normalization::{
        normalize_proof_provenance_with_plan, ConsumedProofAuthority, MirProofNormalizationResult,
        MirProofNormalizationStatistics, MirProofTransitionPlan,
    },
    reachability_verification_errors,
};
use crate::passes::{
    reachability::{
        analyze_reachability, verify_active_lifecycle_reachability, verify_reachable_definitions,
        verify_reachable_static_accesses, MirReachabilityAnalysis,
    },
    static_lifecycle,
};

/// Read-only proof-rich MIR accepted by pre-normalization transformations.
///
/// Construction and seal invalidation are compiler-private. External code
/// can borrow this product from a proof-rich inspection checkpoint, but it
/// cannot forge one:
///
/// ```compile_fail
/// use skald_compiler::{mir::MirProgram, passes::VerifiedProofMirProgram};
///
/// fn forge(program: MirProgram) -> VerifiedProofMirProgram {
///     VerifiedProofMirProgram { program }
/// }
/// ```
///
/// Nor can external callers invoke the proof verifier:
///
/// ```compile_fail
/// use skald_compiler::{mir::MirProgram, passes::verify_proof_mir};
///
/// fn verify(program: MirProgram) { let _ = verify_proof_mir(program); }
/// ```
///
/// A seal borrowed from inspection cannot be invalidated. Proof-rich
/// checkpoints also expose no final reachability facts:
///
/// ```compile_fail
/// use skald_compiler::passes::MirProofPipelineCheckpoint;
///
/// fn invalidate(checkpoint: MirProofPipelineCheckpoint<'_>) {
///     checkpoint.verified().clone().invalidate_for_proof_transformation();
/// }
/// ```
///
/// ```compile_fail
/// use skald_compiler::passes::MirProofPipelineCheckpoint;
///
/// fn detach(checkpoint: MirProofPipelineCheckpoint<'_>) {
///     let _ = checkpoint.verified().reachability();
/// }
/// ```
///
/// Proof-rich MIR also cannot be sent directly to backend input:
///
/// ```compile_fail
/// use skald_compiler::{backend::BackendInput, passes::MirProofPipelineCheckpoint};
///
/// fn skip_normalization(checkpoint: MirProofPipelineCheckpoint<'_>) {
///     let _ = BackendInput::without_runtime_trace(checkpoint.verified());
/// }
/// ```
///
/// The proof-consuming transition capability is pipeline-private, so external
/// callers cannot forge a path around mandatory normalization:
///
/// ```compile_fail
/// use skald_compiler::passes::MirProofTransitionCapability;
/// ```
#[derive(Clone, Eq, PartialEq)]
pub struct VerifiedProofMirProgram {
    program: MirProgram,
}

impl VerifiedProofMirProgram {
    pub const fn program(&self) -> &MirProgram {
        &self.program
    }

    pub(super) fn invalidate_for_proof_transformation(self) -> MirProgram {
        self.program
    }
}

impl fmt::Debug for VerifiedProofMirProgram {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedProofMirProgram")
            .field("program", &self.program)
            .finish()
    }
}

impl Deref for VerifiedProofMirProgram {
    type Target = MirProgram;

    fn deref(&self) -> &Self::Target {
        self.program()
    }
}

/// Read-only normalized final MIR with fresh target-independent reachability.
///
/// The private representation is the backend trust token. It includes
/// normalization authority which can only be issued by the one-way proof
/// provenance transaction. External code cannot forge the seal:
///
/// ```compile_fail
/// use skald_compiler::{mir::MirProgram, passes::VerifiedFinalMirProgram};
///
/// fn forge(program: MirProgram) -> VerifiedFinalMirProgram {
///     VerifiedFinalMirProgram { program }
/// }
/// ```
///
/// Seal-bound reachability facts cannot be detached, replaced, or mutated:
///
/// ```compile_fail
/// use skald_compiler::passes::VerifiedFinalMirProgram;
///
/// fn detach(verified: &VerifiedFinalMirProgram) {
///     let _ = verified.reachability();
/// }
/// ```
///
/// ```compile_fail
/// use skald_compiler::passes::VerifiedFinalMirProgram;
///
/// fn replace_facts(verified: &mut VerifiedFinalMirProgram) {
///     verified.reachability = verified.reachability.clone();
/// }
/// ```
///
/// Raw final-stage invalidation and the consumed-proof token remain private:
///
/// ```compile_fail
/// use skald_compiler::passes::VerifiedFinalMirProgram;
///
/// fn invalidate(verified: VerifiedFinalMirProgram) {
///     let _ = verified.invalidate_for_final_transformation();
/// }
/// ```
///
/// ```compile_fail
/// use skald_compiler::passes::ConsumedProofAuthority;
/// ```
#[derive(Clone, Eq, PartialEq)]
pub struct VerifiedFinalMirProgram {
    program: MirProgram,
    reachability: Box<MirReachabilityAnalysis>,
    _consumed_proof: ConsumedProofAuthority,
}

impl VerifiedFinalMirProgram {
    pub const fn program(&self) -> &MirProgram {
        &self.program
    }

    pub(crate) const fn reachability(&self) -> &MirReachabilityAnalysis {
        &self.reachability
    }

    pub(super) fn invalidate_for_final_transformation(self) -> UnverifiedFinalMirProgram {
        let Self {
            program,
            reachability: _,
            _consumed_proof,
        } = self;
        UnverifiedFinalMirProgram {
            program,
            consumed_proof: _consumed_proof,
        }
    }
}

/// Normalized MIR whose seal-bound analyses were invalidated by a final-stage
/// transformation. The consumed-proof authority travels with the raw program
/// and can only be consumed by normalized resealing.
pub(in crate::passes::pipeline) struct UnverifiedFinalMirProgram {
    program: MirProgram,
    consumed_proof: ConsumedProofAuthority,
}

impl UnverifiedFinalMirProgram {
    pub(in crate::passes::pipeline) fn into_parts(self) -> (MirProgram, ConsumedProofAuthority) {
        (self.program, self.consumed_proof)
    }

    pub(in crate::passes::pipeline) fn from_parts(
        program: MirProgram,
        consumed_proof: ConsumedProofAuthority,
    ) -> Self {
        Self {
            program,
            consumed_proof,
        }
    }
}

impl fmt::Debug for VerifiedFinalMirProgram {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedFinalMirProgram")
            .field("program", &self.program)
            .finish()
    }
}

impl Deref for VerifiedFinalMirProgram {
    type Target = MirProgram;

    fn deref(&self) -> &Self::Target {
        self.program()
    }
}

/// Verifies the complete proof-rich contract without exposing construction or
/// invalidation outside the compiler.
pub(crate) fn verify_proof_mir(
    program: MirProgram,
) -> Result<VerifiedProofMirProgram, MirVerificationErrors> {
    static_lifecycle::verify_synthesized_mir(&program)?;
    let reachability = analyze_reachability(&program).map_err(reachability_verification_errors)?;
    verify_reachable_definitions(&program, &reachability)?;
    verify_active_lifecycle_reachability(&program, &reachability)?;
    verify_reachable_static_accesses(&program, &reachability)?;
    Ok(VerifiedProofMirProgram { program })
}

/// Performs the mandatory one-way transition and seals normalized MIR with
/// reachability facts computed from that exact representation.
pub(super) fn finalize_proof_mir(
    verified: VerifiedProofMirProgram,
) -> Result<(VerifiedFinalMirProgram, MirProofNormalizationStatistics), MirVerificationErrors> {
    transition_proof_mir(verified, None).map_err(MirProofTransitionError::into_verification_errors)
}

/// Failure from one atomic proof-consuming boundary execution.
pub(in crate::passes::pipeline) enum MirProofTransitionError {
    Normalization(MirVerificationErrors),
    FinalVerification(MirVerificationErrors),
}

impl MirProofTransitionError {
    pub(in crate::passes::pipeline) fn into_verification_errors(self) -> MirVerificationErrors {
        match self {
            Self::Normalization(errors) => errors,
            Self::FinalVerification(errors) => errors,
        }
    }

    #[cfg(test)]
    pub(in crate::passes::pipeline) fn normalization_for_test(message: &str) -> Self {
        Self::Normalization(MirVerificationErrors::program(message))
    }

    #[cfg(test)]
    pub(in crate::passes::pipeline) fn final_verification_for_test(message: &str) -> Self {
        Self::FinalVerification(MirVerificationErrors::program(message))
    }
}

pub(in crate::passes::pipeline) fn transition_proof_mir(
    verified: VerifiedProofMirProgram,
    optional_plan: Option<MirProofTransitionPlan>,
) -> Result<(VerifiedFinalMirProgram, MirProofNormalizationStatistics), MirProofTransitionError> {
    let normalized =
        normalize_proof_provenance_with_plan(verified, optional_plan).map_err(|error| {
            MirProofTransitionError::Normalization(normalization_verification_errors(&error))
        })?;
    seal_normalized_mir(normalized).map_err(MirProofTransitionError::FinalVerification)
}

/// Re-establishes normalized structure and fresh seal-bound reachability after
/// one changed final-stage pass. The private authority prevents raw or
/// proof-rich MIR from entering this path.
pub(super) fn reseal_final_mir(
    unverified: UnverifiedFinalMirProgram,
) -> Result<VerifiedFinalMirProgram, MirVerificationErrors> {
    let (program, authority) = unverified.into_parts();
    verify_normalized_program(program, authority)
}

/// Public verify-and-normalize convenience. No API named final returns the
/// proof-rich intermediate.
pub fn verify_final_mir(
    program: MirProgram,
) -> Result<VerifiedFinalMirProgram, MirVerificationErrors> {
    let verified = verify_proof_mir(program)?;
    finalize_proof_mir(verified).map(|(verified, _statistics)| verified)
}

fn seal_normalized_mir(
    normalized: MirProofNormalizationResult,
) -> Result<(VerifiedFinalMirProgram, MirProofNormalizationStatistics), MirVerificationErrors> {
    let (program, statistics, authority) = normalized.into_sealed_parts();
    verify_normalized_program(program, authority).map(|verified| (verified, statistics))
}

fn verify_normalized_program(
    program: MirProgram,
    authority: ConsumedProofAuthority,
) -> Result<VerifiedFinalMirProgram, MirVerificationErrors> {
    static_lifecycle::verify_normalized_synthesized_mir(&program)?;
    let reachability = analyze_reachability(&program).map_err(reachability_verification_errors)?;
    verify_reachable_definitions(&program, &reachability)?;
    verify_active_lifecycle_reachability(&program, &reachability)?;
    verify_reachable_static_accesses(&program, &reachability)?;
    Ok(VerifiedFinalMirProgram {
        program,
        reachability: Box::new(reachability),
        _consumed_proof: authority,
    })
}

fn normalization_verification_errors(
    error: &super::normalization::MirProofNormalizationError,
) -> MirVerificationErrors {
    MirVerificationErrors::program(format!("proof-provenance normalization failed: {error}"))
}
