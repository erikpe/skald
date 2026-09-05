//! Atomic conversion from proof-rich to normalized executable MIR.
//!
//! The transaction is intentionally not a selectable pass. It first builds a
//! complete immutable plan for every executable definition, then consumes the
//! verified input through the existing all-program dense rewrite boundary.
//! The result remains private and can only be consumed by final-seal
//! construction, so no raw normalized program can cross the trust boundary.

use crate::mir::{
    check_normalized_mir,
    rewrite::{rewrite_program, MirCallableEdit, MirRewriteError},
    MirProgram, MirVerificationErrors,
};

use super::{
    optimizations::constant_short_circuit_folding::LogicalSelectionPlan, VerifiedProofMirProgram,
};

mod error;
mod plan;

pub(super) use error::MirProofNormalizationError;
use error::MirProofNormalizationErrorKind;

/// Optional proof-aware edits which may be composed with mandatory
/// normalization.
///
pub(in crate::passes::pipeline) enum MirProofTransitionPlan {
    Logical(LogicalSelectionPlan),
}

impl MirProofTransitionPlan {
    pub(in crate::passes::pipeline) const fn logical(plan: LogicalSelectionPlan) -> Self {
        Self::Logical(plan)
    }

    pub(in crate::passes::pipeline) fn processed_callables(&self) -> usize {
        match self {
            Self::Logical(plan) => plan.processed_callables(),
        }
    }

    pub(in crate::passes::pipeline) fn changed_callable_count(&self) -> usize {
        match self {
            Self::Logical(plan) => plan.changed_callable_count(),
        }
    }

    fn validate_program(&self, program: &MirProgram) -> Result<(), MirRewriteError> {
        match self {
            Self::Logical(plan) => plan.validate_program(program),
        }
    }

    fn apply_callable(
        &self,
        callable: crate::identity::CallableId,
        edit: &mut MirCallableEdit,
    ) -> Result<(), MirRewriteError> {
        match self {
            Self::Logical(plan) => plan.apply_callable(callable, edit),
        }
    }
}

pub(in crate::passes::pipeline) enum MirProofTransitionNormalizationError {
    OptionalPlanRewrite(MirRewriteError),
    OptionalPlanVerification(MirVerificationErrors),
    Normalization(MirProofNormalizationError),
}

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
#[cfg(test)]
pub(in crate::passes::pipeline) fn normalize_proof_provenance(
    verified: VerifiedProofMirProgram,
) -> Result<MirProofNormalizationResult, MirProofNormalizationError> {
    normalize_program(verified.invalidate_for_proof_transformation())
}

pub(in crate::passes::pipeline) fn normalize_proof_provenance_with_plan(
    verified: VerifiedProofMirProgram,
    optional_plan: Option<MirProofTransitionPlan>,
) -> Result<MirProofNormalizationResult, MirProofTransitionNormalizationError> {
    normalize_program_with_plan(
        verified.invalidate_for_proof_transformation(),
        optional_plan,
    )
}

#[cfg(test)]
fn normalize_program(
    program: MirProgram,
) -> Result<MirProofNormalizationResult, MirProofNormalizationError> {
    match normalize_program_with_plan(program, None) {
        Ok(normalized) => Ok(normalized),
        Err(MirProofTransitionNormalizationError::Normalization(error)) => Err(error),
        Err(
            MirProofTransitionNormalizationError::OptionalPlanRewrite(_)
            | MirProofTransitionNormalizationError::OptionalPlanVerification(_),
        ) => unreachable!("the core normalization path has no optional plan"),
    }
}

fn normalize_program_with_plan(
    program: MirProgram,
    optional_plan: Option<MirProofTransitionPlan>,
) -> Result<MirProofNormalizationResult, MirProofTransitionNormalizationError> {
    if let Some(plan) = &optional_plan {
        plan.validate_program(&program)
            .map_err(MirProofTransitionNormalizationError::OptionalPlanRewrite)?;
    }
    let plans = plan::inventory_program(&program)
        .map_err(MirProofTransitionNormalizationError::Normalization)?;
    let mut statistics = MirProofNormalizationStatistics::default();
    for plan in &plans {
        statistics.add(plan.statistics());
    }

    let mut plans = plans.into_iter();
    let has_optional_plan = optional_plan.is_some();
    let mut optional_plan_failed = false;
    let mut normalization_plan_failed = false;
    let rewrite = rewrite_program(program, |callable, edit| {
        if let Some(plan) = &optional_plan {
            if let Err(error) = plan.apply_callable(callable, edit) {
                optional_plan_failed = true;
                return Err(error);
            }
        }
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
        if let Err(error) = plan.apply(edit) {
            normalization_plan_failed = true;
            return Err(error);
        }
        Ok(())
    })
    .map_err(|error| {
        if normalization_plan_failed {
            MirProofTransitionNormalizationError::Normalization(error.into())
        } else if optional_plan_failed || has_optional_plan {
            MirProofTransitionNormalizationError::OptionalPlanRewrite(error)
        } else {
            MirProofTransitionNormalizationError::Normalization(error.into())
        }
    })?;
    debug_assert!(plans.next().is_none());

    check_normalized_mir(&rewrite.program).map_err(|errors| {
        if has_optional_plan {
            MirProofTransitionNormalizationError::OptionalPlanVerification(errors)
        } else {
            MirProofTransitionNormalizationError::Normalization(
                MirProofNormalizationErrorKind::NormalizedInvariant(errors).into(),
            )
        }
    })?;
    Ok(MirProofNormalizationResult {
        program: rewrite.program,
        statistics,
        authority: ConsumedProofAuthority { _private: () },
    })
}

#[cfg(test)]
mod tests;
