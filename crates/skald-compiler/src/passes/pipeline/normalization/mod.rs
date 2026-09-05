//! Atomic conversion from proof-rich to normalized executable MIR.
//!
//! The transaction is intentionally not a selectable pass. It first builds a
//! complete immutable plan for every executable definition, then consumes the
//! verified input through the existing all-program dense rewrite boundary.
//! The result remains private and can only be consumed by final-seal
//! construction, so no raw normalized program can cross the trust boundary.

use crate::mir::{
    check_normalized_mir,
    rewrite::{rewrite_program, MirRewriteError},
    MirProgram,
};

use super::VerifiedProofMirProgram;

mod error;
mod plan;

pub(super) use error::MirProofNormalizationError;
use error::MirProofNormalizationErrorKind;

/// Optional proof-aware edits which may be composed with mandatory
/// normalization.
///
/// CLR5 deliberately leaves this type uninhabited. The transition API accepts
/// it now so adding the reviewed logical-selection plan does not require
/// widening the capability later; CLR6 will add the first private variant.
pub(in crate::passes::pipeline) enum MirProofTransitionPlan {}

/// Deterministic structural accounting for the mandatory conversion.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct MirProofNormalizationStatistics {
    path_condition_records: usize,
    logical_expression_records: usize,
    path_reads: usize,
    activation_storage: usize,
    changed_callables: usize,
    released_proof_blocks: usize,
}

impl MirProofNormalizationStatistics {
    const fn new(
        path_condition_records: usize,
        logical_expression_records: usize,
        path_reads: usize,
        activation_storage: usize,
        changed_callables: usize,
        released_proof_blocks: usize,
    ) -> Self {
        Self {
            path_condition_records,
            logical_expression_records,
            path_reads,
            activation_storage,
            changed_callables,
            released_proof_blocks,
        }
    }

    fn add(&mut self, other: Self) {
        self.path_condition_records = self
            .path_condition_records
            .saturating_add(other.path_condition_records);
        self.logical_expression_records = self
            .logical_expression_records
            .saturating_add(other.logical_expression_records);
        self.path_reads = self.path_reads.saturating_add(other.path_reads);
        self.activation_storage = self
            .activation_storage
            .saturating_add(other.activation_storage);
        self.changed_callables = self
            .changed_callables
            .saturating_add(other.changed_callables);
        self.released_proof_blocks = self
            .released_proof_blocks
            .saturating_add(other.released_proof_blocks);
    }

    pub(crate) const fn path_condition_records(self) -> usize {
        self.path_condition_records
    }

    pub(crate) const fn logical_expression_records(self) -> usize {
        self.logical_expression_records
    }

    pub(crate) const fn path_reads(self) -> usize {
        self.path_reads
    }

    pub(crate) const fn activation_storage(self) -> usize {
        self.activation_storage
    }

    pub(crate) const fn changed_callables(self) -> usize {
        self.changed_callables
    }

    pub(crate) const fn released_proof_blocks(self) -> usize {
        self.released_proof_blocks
    }
}

/// Successfully normalized raw MIR awaiting final-seal construction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::passes::pipeline) struct MirProofNormalizationResult {
    program: MirProgram,
    statistics: MirProofNormalizationStatistics,
    authority: ConsumedProofAuthority,
}

/// Unforgeable evidence that the exact proof-rich input passed through the
/// complete normalization transaction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ConsumedProofAuthority {
    _private: (),
}

impl MirProofNormalizationResult {
    #[cfg(test)]
    pub(in crate::passes::pipeline) const fn program(&self) -> &MirProgram {
        &self.program
    }

    #[cfg(test)]
    pub(in crate::passes::pipeline) const fn statistics(&self) -> MirProofNormalizationStatistics {
        self.statistics
    }

    pub(super) fn into_sealed_parts(
        self,
    ) -> (
        MirProgram,
        MirProofNormalizationStatistics,
        ConsumedProofAuthority,
    ) {
        (self.program, self.statistics, self.authority)
    }
}

/// Consumes a proof-verified product and atomically removes its proof-only
/// path and logical provenance.
#[allow(dead_code)]
pub(in crate::passes::pipeline) fn normalize_proof_provenance(
    verified: VerifiedProofMirProgram,
) -> Result<MirProofNormalizationResult, MirProofNormalizationError> {
    normalize_proof_provenance_with_plan(verified, None)
}

pub(in crate::passes::pipeline) fn normalize_proof_provenance_with_plan(
    verified: VerifiedProofMirProgram,
    optional_plan: Option<MirProofTransitionPlan>,
) -> Result<MirProofNormalizationResult, MirProofNormalizationError> {
    if let Some(plan) = optional_plan {
        match plan {}
    }
    normalize_program(verified.invalidate_for_proof_transformation())
}

fn normalize_program(
    program: MirProgram,
) -> Result<MirProofNormalizationResult, MirProofNormalizationError> {
    let plans = plan::inventory_program(&program)?;
    let mut statistics = MirProofNormalizationStatistics::default();
    for plan in &plans {
        statistics.add(plan.statistics());
    }

    let mut plans = plans.into_iter();
    let rewrite = rewrite_program(program, |callable, edit| {
        let Some(plan) = plans.next() else {
            return Err(MirRewriteError::StaleCallableSnapshot {
                callable,
                subject: "proof-normalization plan",
            });
        };
        if plan.callable() != callable {
            return Err(MirRewriteError::StaleCallableSnapshot {
                callable,
                subject: "proof-normalization plan order",
            });
        }
        plan.apply(edit)
    })?;
    debug_assert!(plans.next().is_none());

    check_normalized_mir(&rewrite.program).map_err(|errors| {
        MirProofNormalizationError::from(MirProofNormalizationErrorKind::NormalizedInvariant(
            errors,
        ))
    })?;
    Ok(MirProofNormalizationResult {
        program: rewrite.program,
        statistics,
        authority: ConsumedProofAuthority { _private: () },
    })
}

#[cfg(test)]
mod tests;
