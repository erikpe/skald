//! Exact cleanup of dead normalized path-activation carrier protocols.

use super::super::{
    execution::{
        MirFinalPassCapability, MirFinalPassOutcome, MirFinalStorageCleanupPlan, MirPassData,
        MirPassFailure, MirPassMeasurement,
    },
    policy::{MirPassDescriptor, MirPassImplementation, MirPassRegistration},
    MirPassIdentity, MirPassStage,
};

pub(in crate::passes::pipeline) const IDENTITY: MirPassIdentity = MirPassIdentity::new(12);
const NAME: &str = "dead-normalized-path-activation-cleanup";
const DESCRIPTION: &str = "Removes complete unused normalized path-activation protocols.";
const INSPECTED_CARRIERS: &str = "inspected normalized activation carriers";
const REMOVABLE_CARRIERS: &str = "removable normalized activation carriers";
const PROTECTED_CARRIERS: &str = "protected normalized activation carriers";
const REMOVED_STORAGES: &str = "removed storage declarations";
const REMOVED_LOADS: &str = "removed load instructions";
const REMOVED_STORES: &str = "removed store instructions";
const REMOVED_LIFETIME_MARKERS: &str = "removed lifetime markers";
const REMOVED_VALUES: &str = "removed value declarations";
const MAXIMUM_PROTOCOL_SIZE: &str = "maximum removable protocol size";

pub(in crate::passes::pipeline) const REGISTRATION: MirPassRegistration = MirPassRegistration::new(
    MirPassDescriptor::new(IDENTITY, MirPassStage::Final, NAME, DESCRIPTION),
    MirPassImplementation::final_stage(IDENTITY, transform),
);

fn transform(capability: MirFinalPassCapability) -> Result<MirFinalPassOutcome, MirPassFailure> {
    let plan = MirFinalStorageCleanupPlan::prepare(capability.verified())
        .map_err(MirPassFailure::Rewrite)?;
    let data = pass_data(&plan);
    if plan.is_empty() {
        return capability.unchanged_with(data);
    }

    capability.cleanup_dead_path_activations(plan)?.finish(data)
}

fn pass_data(plan: &MirFinalStorageCleanupPlan) -> MirPassData {
    let data = if plan.is_empty() {
        MirPassData::processed(plan.processed_callables())
    } else {
        MirPassData::changed(plan.changed_callables())
    };
    let removed = plan.summary();
    data.with_measurement(MirPassMeasurement::count(
        INSPECTED_CARRIERS,
        plan.inspected_carriers(),
    ))
    .with_measurement(MirPassMeasurement::count(
        REMOVABLE_CARRIERS,
        plan.removable_carriers(),
    ))
    .with_measurement(MirPassMeasurement::count(
        PROTECTED_CARRIERS,
        plan.protected_carriers(),
    ))
    .with_measurement(MirPassMeasurement::count(
        REMOVED_STORAGES,
        count(removed.storages()),
    ))
    .with_measurement(MirPassMeasurement::count(
        REMOVED_LOADS,
        count(removed.loads()),
    ))
    .with_measurement(MirPassMeasurement::count(
        REMOVED_STORES,
        count(removed.stores()),
    ))
    .with_measurement(MirPassMeasurement::count(
        REMOVED_LIFETIME_MARKERS,
        count(removed.lifetime_markers()),
    ))
    .with_measurement(MirPassMeasurement::count(
        REMOVED_VALUES,
        count(removed.values()),
    ))
    .with_measurement(MirPassMeasurement::count(
        MAXIMUM_PROTOCOL_SIZE,
        plan.maximum_protocol_size(),
    ))
}

fn count(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

#[cfg(test)]
#[path = "dead_normalized_path_activation_cleanup/tests.rs"]
mod tests;
