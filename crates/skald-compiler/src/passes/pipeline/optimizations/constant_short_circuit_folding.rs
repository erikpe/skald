//! Constant-left selection of verified short-circuit logical protocols.

#[path = "constant_short_circuit_folding/plan.rs"]
mod plan;

use super::super::{
    execution::{
        MirPassData, MirPassFailure, MirPassMeasurement, MirProofTransitionCapability,
        MirProofTransitionFailure, MirProofTransitionOutcome,
    },
    normalization::MirProofTransitionPlan,
    policy::{MirPassDescriptor, MirPassImplementation, MirPassRegistration},
    MirPassIdentity, MirPassStage,
};

pub(in crate::passes::pipeline) use plan::LogicalSelectionPlan;
use plan::LogicalSelectionPlanError;

pub(in crate::passes::pipeline) const IDENTITY: MirPassIdentity = MirPassIdentity::new(9);
const NAME: &str = "constant-short-circuit-folding";
const DESCRIPTION: &str =
    "Selects exact short-circuit paths whose left result is a convergent constant.";
const SELECTED_AND_SHORT_PATHS: &str = "selected && short paths";
const SELECTED_AND_RIGHT_PATHS: &str = "selected && right paths";
const SELECTED_OR_SHORT_PATHS: &str = "selected || short paths";
const SELECTED_OR_RIGHT_PATHS: &str = "selected || right paths";
const REPLACED_SELECTED_RESULTS: &str = "replaced selected-result loads";

pub(in crate::passes::pipeline) const REGISTRATION: MirPassRegistration = MirPassRegistration::new(
    MirPassDescriptor::new(IDENTITY, MirPassStage::ProofTransition, NAME, DESCRIPTION),
    MirPassImplementation::proof_transition(IDENTITY, transform),
);

fn transform(
    capability: MirProofTransitionCapability,
) -> Result<MirProofTransitionOutcome, MirProofTransitionFailure> {
    let plan =
        LogicalSelectionPlan::prepare(capability.verified().program()).map_err(plan_failure)?;
    let data = if plan.is_empty() {
        MirPassData::processed(plan.processed_callables())
    } else {
        MirPassData::processed_and_changed(
            plan.processed_callables(),
            plan.changed_callable_count(),
        )
    };
    let data = pass_data(data, &plan);
    if plan.is_empty() {
        capability.normalize(None, data)
    } else {
        capability.normalize(Some(MirProofTransitionPlan::logical(plan)), data)
    }
}

fn pass_data(data: MirPassData, plan: &LogicalSelectionPlan) -> MirPassData {
    let counts = plan.counts();
    data.with_measurement(MirPassMeasurement::count(
        SELECTED_AND_SHORT_PATHS,
        count(counts.and_short),
    ))
    .with_measurement(MirPassMeasurement::count(
        SELECTED_AND_RIGHT_PATHS,
        count(counts.and_right),
    ))
    .with_measurement(MirPassMeasurement::count(
        SELECTED_OR_SHORT_PATHS,
        count(counts.or_short),
    ))
    .with_measurement(MirPassMeasurement::count(
        SELECTED_OR_RIGHT_PATHS,
        count(counts.or_right),
    ))
    .with_measurement(MirPassMeasurement::count(
        REPLACED_SELECTED_RESULTS,
        count(counts.replaced_selected_results),
    ))
}

fn count(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

fn plan_failure(error: LogicalSelectionPlanError) -> MirProofTransitionFailure {
    let failure = match error {
        LogicalSelectionPlanError::Rewrite(error) => MirPassFailure::Rewrite(error),
        LogicalSelectionPlanError::Analysis(error) => MirPassFailure::execution(error.to_string()),
        LogicalSelectionPlanError::RejectedTopology { .. }
        | LogicalSelectionPlanError::MissingTopology { .. }
        | LogicalSelectionPlanError::InconsistentSelection { .. }
        | LogicalSelectionPlanError::ConflictingCandidates { .. } => {
            MirPassFailure::execution(error.to_string())
        }
    };
    failure.into()
}

#[cfg(test)]
#[path = "constant_short_circuit_folding/tests.rs"]
mod tests;
