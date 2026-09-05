//! Convergent folding of verified checked-integer protocols.

#[path = "checked_integer_folding/plan.rs"]
mod plan;

use super::super::{
    execution::{
        MirPassData, MirPassFailure, MirPassMeasurement, MirProofPassCapability,
        MirProofPassOutcome,
    },
    policy::{MirPassDescriptor, MirPassImplementation, MirPassRegistration},
    MirPassIdentity, MirPassStage,
};
use plan::CheckedIntegerFoldPlanError;
pub(super) use plan::{CheckedIntegerFoldPlan, CheckedIntegerFoldSelection};

pub(in crate::passes::pipeline) const IDENTITY: MirPassIdentity = MirPassIdentity::new(5);
const NAME: &str = "checked-integer-constant-folding";
const DESCRIPTION: &str = "Folds exact successful checked-integer protocols from convergent facts.";
const FOLDED_QUOTIENTS: &str = "folded quotient protocols";
const FOLDED_REMAINDERS: &str = "folded remainder protocols";
const FOLDED_SHIFTS: &str = "folded shift protocols";
const PROPAGATED_OPERAND_FOLDS: &str = "folded protocols with propagated operands";
const REMOVED_PROTOCOL_LOAD_VALUES: &str = "removed protocol-load values";
const RETAINED_STATIC_FAILURES: &str = "retained statically failing candidates";

pub(in crate::passes::pipeline) const REGISTRATION: MirPassRegistration = MirPassRegistration::new(
    MirPassDescriptor::new(IDENTITY, MirPassStage::ProofRich, NAME, DESCRIPTION),
    MirPassImplementation::proof_rich(IDENTITY, transform),
);

fn transform(capability: MirProofPassCapability) -> Result<MirProofPassOutcome, MirPassFailure> {
    let plan = CheckedIntegerFoldPlan::prepare(
        capability.verified().program(),
        CheckedIntegerFoldSelection::All,
    )
    .map_err(plan_failure)?;
    if plan.is_empty() {
        return capability.unchanged_with(pass_data(&plan, 0, 0));
    }

    let changed_callables = plan.changed_callable_count();
    let mut removed_protocol_load_values = 0usize;
    let rewritten = capability.rewrite(|callable, edit| {
        removed_protocol_load_values =
            removed_protocol_load_values.saturating_add(plan.rewrite_callable(callable, edit)?);
        Ok(())
    })?;
    rewritten.finish(pass_data(
        &plan,
        changed_callables,
        removed_protocol_load_values,
    ))
}

fn plan_failure(error: CheckedIntegerFoldPlanError) -> MirPassFailure {
    match error {
        CheckedIntegerFoldPlanError::Rewrite(error) => MirPassFailure::Rewrite(error),
        CheckedIntegerFoldPlanError::Analysis(error) => {
            MirPassFailure::execution(error.to_string())
        }
        CheckedIntegerFoldPlanError::ConflictingCandidates { .. } => {
            MirPassFailure::execution(error.to_string())
        }
    }
}

fn pass_data(
    plan: &CheckedIntegerFoldPlan,
    changed_callables: usize,
    removed_protocol_load_values: usize,
) -> MirPassData {
    let data = if changed_callables == 0 {
        MirPassData::processed(plan.processed_callables())
    } else {
        MirPassData::changed(changed_callables)
    };
    let counts = plan.counts();
    data.with_measurement(MirPassMeasurement::count(
        FOLDED_QUOTIENTS,
        count(counts.quotients),
    ))
    .with_measurement(MirPassMeasurement::count(
        FOLDED_REMAINDERS,
        count(counts.remainders),
    ))
    .with_measurement(MirPassMeasurement::count(
        FOLDED_SHIFTS,
        count(counts.shifts),
    ))
    .with_measurement(MirPassMeasurement::count(
        PROPAGATED_OPERAND_FOLDS,
        count(counts.propagated_operand_folds),
    ))
    .with_measurement(MirPassMeasurement::count(
        REMOVED_PROTOCOL_LOAD_VALUES,
        count(removed_protocol_load_values),
    ))
    .with_measurement(MirPassMeasurement::count(
        RETAINED_STATIC_FAILURES,
        count(counts.retained_static_failures),
    ))
}

fn count(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

#[cfg(test)]
#[path = "checked_integer_folding/tests.rs"]
mod tests;
