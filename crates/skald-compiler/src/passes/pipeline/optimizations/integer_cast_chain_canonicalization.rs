//! Exact canonicalization of same-block integer cast chains.

#[path = "integer_cast_chain_canonicalization/plan.rs"]
mod plan;

use super::super::{
    execution::{
        MirPassData, MirPassFailure, MirPassMeasurement, MirProofPassContext, MirProofPassOutcome,
    },
    policy::{MirPassDescriptor, MirPassImplementation, MirPassRegistration},
    MirPassIdentity, MirPassStage,
};
use plan::IntegerCastCanonicalizationPlan;

pub(in crate::passes::pipeline) const IDENTITY: MirPassIdentity = MirPassIdentity::new(11);
const NAME: &str = "integer-cast-chain-canonicalization";
const DESCRIPTION: &str = "Replaces integer cast-chain endpoints with exact shortest recipes.";
const RETARGETED_ENDPOINTS: &str = "retargeted cast endpoints";
const FORWARDED_ENDPOINTS: &str = "forwarded identity endpoints";
const FORWARDED_USES: &str = "forwarded value uses";
const REMOVED_ASSIGNMENTS: &str = "removed assignment instructions";
const REMOVED_VALUES: &str = "removed value declarations";
const ELIMINATED_STEPS: &str = "eliminated cast steps";
const PROTECTED_REJECTIONS: &str = "rejected protected candidates";
const MAXIMUM_DEPTH: &str = "maximum rewritten chain depth";

pub(in crate::passes::pipeline) const REGISTRATION: MirPassRegistration = MirPassRegistration::new(
    MirPassDescriptor::new(IDENTITY, MirPassStage::ProofRich, NAME, DESCRIPTION),
    MirPassImplementation::proof_rich(IDENTITY, transform),
);

fn transform(capability: MirProofPassContext) -> Result<MirProofPassOutcome, MirPassFailure> {
    let plan = IntegerCastCanonicalizationPlan::prepare(capability.verified().program())
        .map_err(MirPassFailure::Rewrite)?;
    if plan.is_empty() {
        return capability.unchanged_with(pass_data(&plan, 0));
    }

    // Validate every selected dense source before opening the first sparse
    // transaction. Each callable is checked again immediately before edits.
    plan.validate_program(capability.verified().program())
        .map_err(MirPassFailure::Rewrite)?;
    let mut forwarded_uses = 0usize;
    let rewritten = capability.rewrite(|callable, edit| {
        forwarded_uses = forwarded_uses.saturating_add(plan.rewrite_callable(callable, edit)?);
        Ok(())
    })?;
    rewritten.finish(pass_data(&plan, forwarded_uses))
}

fn pass_data(plan: &IntegerCastCanonicalizationPlan, forwarded_uses: usize) -> MirPassData {
    let data = if plan.is_empty() {
        MirPassData::processed(plan.processed_callables())
    } else {
        MirPassData::changed(plan.changed_callables())
    };
    let counts = plan.counts();
    data.with_measurement(MirPassMeasurement::count(
        RETARGETED_ENDPOINTS,
        count(counts.retargeted_endpoints),
    ))
    .with_measurement(MirPassMeasurement::count(
        FORWARDED_ENDPOINTS,
        count(counts.forwarded_endpoints),
    ))
    .with_measurement(MirPassMeasurement::count(
        FORWARDED_USES,
        count(forwarded_uses),
    ))
    .with_measurement(MirPassMeasurement::count(
        REMOVED_ASSIGNMENTS,
        count(counts.forwarded_endpoints),
    ))
    .with_measurement(MirPassMeasurement::count(
        REMOVED_VALUES,
        count(counts.forwarded_endpoints),
    ))
    .with_measurement(MirPassMeasurement::count(
        ELIMINATED_STEPS,
        count(counts.eliminated_steps),
    ))
    .with_measurement(MirPassMeasurement::count(
        PROTECTED_REJECTIONS,
        count(counts.protected_rejections),
    ))
    .with_measurement(MirPassMeasurement::count(
        MAXIMUM_DEPTH,
        count(counts.maximum_depth),
    ))
}

fn count(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

#[cfg(test)]
#[path = "integer_cast_chain_canonicalization/tests.rs"]
mod tests;
