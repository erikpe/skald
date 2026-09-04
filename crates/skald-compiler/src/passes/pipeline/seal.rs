//! Proof-rich and normalized final-MIR trust products.

use std::{fmt, ops::Deref};

use crate::mir::{MirProgram, MirVerificationErrors};

use super::{
    normalization::{
        normalize_proof_provenance, ConsumedProofAuthority, MirProofNormalizationResult,
        MirProofNormalizationStatistics,
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
/// A seal borrowed from inspection cannot be invalidated or detached from its
/// reachability facts:
///
/// ```compile_fail
/// use skald_compiler::passes::MirPipelineCheckpoint;
///
/// fn invalidate(checkpoint: MirPipelineCheckpoint<'_>) {
///     checkpoint.verified().clone().invalidate_for_proof_transformation();
/// }
/// ```
///
/// ```compile_fail
/// use skald_compiler::passes::MirPipelineCheckpoint;
///
/// fn detach(checkpoint: MirPipelineCheckpoint<'_>) {
///     let _ = checkpoint.verified().reachability();
/// }
/// ```
///
/// Proof-rich MIR also cannot be sent directly to backend input:
///
/// ```compile_fail
/// use skald_compiler::{backend::BackendInput, passes::MirPipelineCheckpoint};
///
/// fn skip_normalization(checkpoint: MirPipelineCheckpoint<'_>) {
///     let _ = BackendInput::without_runtime_trace(checkpoint.verified());
/// }
/// ```
#[derive(Clone, Eq, PartialEq)]
pub struct VerifiedProofMirProgram {
    program: MirProgram,
    reachability: Box<MirReachabilityAnalysis>,
}

impl VerifiedProofMirProgram {
    pub const fn program(&self) -> &MirProgram {
        &self.program
    }

    pub(crate) const fn reachability(&self) -> &MirReachabilityAnalysis {
        &self.reachability
    }

    pub(super) fn invalidate_for_proof_transformation(self) -> MirProgram {
        let Self {
            program,
            reachability: _,
        } = self;
        program
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

    #[allow(dead_code)]
    pub(super) fn invalidate_for_final_transformation(self) -> MirProgram {
        let Self {
            program,
            reachability: _,
            _consumed_proof: _,
        } = self;
        program
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
    Ok(VerifiedProofMirProgram {
        program,
        reachability: Box::new(reachability),
    })
}

/// Performs the mandatory one-way transition and seals normalized MIR with
/// reachability facts computed from that exact representation.
pub(super) fn finalize_proof_mir(
    verified: VerifiedProofMirProgram,
) -> Result<(VerifiedFinalMirProgram, MirProofNormalizationStatistics), MirVerificationErrors> {
    let normalized = normalize_proof_provenance(verified)
        .map_err(|error| normalization_verification_errors(&error))?;
    seal_normalized_mir(normalized)
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
    static_lifecycle::verify_normalized_synthesized_mir(&program)?;
    let reachability = analyze_reachability(&program).map_err(reachability_verification_errors)?;
    verify_reachable_definitions(&program, &reachability)?;
    verify_active_lifecycle_reachability(&program, &reachability)?;
    verify_reachable_static_accesses(&program, &reachability)?;
    Ok((
        VerifiedFinalMirProgram {
            program,
            reachability: Box::new(reachability),
            _consumed_proof: authority,
        },
        statistics,
    ))
}

fn normalization_verification_errors(
    error: &super::normalization::MirProofNormalizationError,
) -> MirVerificationErrors {
    MirVerificationErrors::program(format!("proof-provenance normalization failed: {error}"))
}
