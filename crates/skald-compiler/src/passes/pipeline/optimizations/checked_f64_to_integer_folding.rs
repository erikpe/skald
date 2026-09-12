//! Convergent folding of verified checked floating-to-integer protocols.

#[path = "checked_f64_to_integer_folding/plan.rs"]
mod plan;

use super::super::{
    execution::{
        MirPassData, MirPassFailure, MirPassMeasurement, MirProofPassContext, MirProofPassOutcome,
    },
    policy::{MirPassDescriptor, MirPassImplementation, MirPassRegistration},
    MirPassIdentity, MirPassStage,
};
pub(super) use plan::CheckedF64ToIntegerFoldPlan;
use plan::CheckedF64ToIntegerFoldPlanError;

pub(in crate::passes::pipeline) const IDENTITY: MirPassIdentity = MirPassIdentity::new(10);
const NAME: &str = "checked-f64-to-integer-constant-folding";
const DESCRIPTION: &str =
    "Folds exact successful checked floating-to-integer protocols from convergent facts.";
const FOLDED_I64: &str = "folded f64-to-i64 protocols";
const FOLDED_U64: &str = "folded f64-to-u64 protocols";
const FOLDED_U8: &str = "folded f64-to-u8 protocols";
const PROPAGATED_SOURCE_FOLDS: &str = "folded protocols with propagated sources";
const REMOVED_PROTOCOL_VALUES: &str = "removed protocol values";
const RETAINED_STATIC_FAILURES: &str = "retained statically failing candidates";

pub(in crate::passes::pipeline) const REGISTRATION: MirPassRegistration = MirPassRegistration::new(
    MirPassDescriptor::new(IDENTITY, MirPassStage::ProofRich, NAME, DESCRIPTION),
    MirPassImplementation::proof_rich(IDENTITY, transform),
);

fn transform(mut capability: MirProofPassContext) -> Result<MirProofPassOutcome, MirPassFailure> {
    let plan =
        CheckedF64ToIntegerFoldPlan::prepare_with_context(&mut capability).map_err(plan_failure)?;
    if plan.is_empty() {
        return capability.unchanged_with(pass_data(&plan, 0, 0));
    }

    let changed_callables = plan.changed_callable_count();
    let mut removed_protocol_values = 0usize;
    let rewritten = capability.rewrite(|callable, edit| {
        removed_protocol_values =
            removed_protocol_values.saturating_add(plan.rewrite_callable(callable, edit)?);
        Ok(())
    })?;
    rewritten.finish(pass_data(&plan, changed_callables, removed_protocol_values))
}

fn plan_failure(error: CheckedF64ToIntegerFoldPlanError) -> MirPassFailure {
    match error {
        CheckedF64ToIntegerFoldPlanError::Rewrite(error) => MirPassFailure::Rewrite(error),
        other => MirPassFailure::execution(other.to_string()),
    }
}

fn pass_data(
    plan: &CheckedF64ToIntegerFoldPlan,
    changed_callables: usize,
    removed_protocol_values: usize,
) -> MirPassData {
    let data = if changed_callables == 0 {
        MirPassData::processed(plan.processed_callables())
    } else {
        MirPassData::changed(changed_callables)
    };
    let counts = plan.counts();
    data.with_measurement(MirPassMeasurement::count(FOLDED_I64, count(counts.i64)))
        .with_measurement(MirPassMeasurement::count(FOLDED_U64, count(counts.u64)))
        .with_measurement(MirPassMeasurement::count(FOLDED_U8, count(counts.u8)))
        .with_measurement(MirPassMeasurement::count(
            PROPAGATED_SOURCE_FOLDS,
            count(counts.propagated_source_folds),
        ))
        .with_measurement(MirPassMeasurement::count(
            REMOVED_PROTOCOL_VALUES,
            count(removed_protocol_values),
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
#[path = "checked_f64_to_integer_folding/tests.rs"]
mod tests;
