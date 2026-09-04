//! Whole-world pruning of unreachable executable definitions.

use crate::{
    mir::retain::{MirDefinitionKindCounts, MirDefinitionRetentionSummary},
    passes::reachability::MirReachabilityCounts,
};

use super::super::{
    execution::{
        MirFinalPassCapability, MirFinalPassOutcome, MirPassData, MirPassFailure,
        MirPassMeasurement,
    },
    policy::{MirPassDescriptor, MirPassImplementation, MirPassRegistration},
    MirPassIdentity, MirPassStage,
};

pub(in crate::passes::pipeline) const IDENTITY: MirPassIdentity = MirPassIdentity::new(1);
const NAME: &str = "whole-world-reachability";
const DESCRIPTION: &str = "Removes unreachable executable MIR definitions.";

pub(in crate::passes::pipeline) const REGISTRATION: MirPassRegistration = MirPassRegistration::new(
    MirPassDescriptor::new(IDENTITY, MirPassStage::Final, NAME, DESCRIPTION),
    MirPassImplementation::final_stage(IDENTITY, transform),
);

fn transform(capability: MirFinalPassCapability) -> Result<MirFinalPassOutcome, MirPassFailure> {
    let reachability = capability.verified().reachability().counts();
    let retention = capability.retain_reachable_definitions()?;
    let data = pass_data(retention.summary(), reachability);
    retention.finish(data)
}

fn pass_data(
    retention: &MirDefinitionRetentionSummary,
    reachability: MirReachabilityCounts,
) -> MirPassData {
    let examined = retention.examined();
    let reachable = retention.retained();
    let removed = retention.removed();
    let mut data = MirPassData::changed(removed.total());

    for (name, value) in definition_measurements(
        [
            "examined definitions",
            "examined function definitions",
            "examined static-initializer definitions",
            "examined member definitions",
        ],
        examined,
    )
    .into_iter()
    .chain(definition_measurements(
        [
            "reachable definitions",
            "reachable function definitions",
            "reachable static-initializer definitions",
            "reachable member definitions",
        ],
        reachable,
    ))
    .chain(definition_measurements(
        [
            "removed definitions",
            "removed function definitions",
            "removed static-initializer definitions",
            "removed member definitions",
        ],
        removed,
    ))
    .chain([
        ("whole-program roots", reachability.roots),
        ("reachable execution nodes", reachability.reachable_nodes),
        ("reachable callables", reachability.reachable_callables),
        ("dependency edges", reachability.dependencies),
        ("runtime entity targets", reachability.runtime_entities),
        ("virtual dispatch families", reachability.virtual_families),
        (
            "interface dispatch requirements",
            reachability.interface_requirements,
        ),
        (
            "function-value signatures",
            reachability.function_value_signatures,
        ),
        (
            "function-value targets",
            reachability.function_value_targets,
        ),
    ]) {
        data = data.with_measurement(MirPassMeasurement::count(name, count(value)));
    }
    data
}

fn definition_measurements(
    names: [&'static str; 4],
    counts: MirDefinitionKindCounts,
) -> [(&'static str, usize); 4] {
    [
        (names[0], counts.total()),
        (names[1], counts.functions()),
        (names[2], counts.static_initializers()),
        (names[3], member_definitions(counts)),
    ]
}

const fn member_definitions(counts: MirDefinitionKindCounts) -> usize {
    counts.initializers()
        + counts.copy_constructors()
        + counts.copy_assignments()
        + counts.destructors()
        + counts.methods()
}

fn count(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

#[cfg(test)]
#[path = "whole_world_reachability/tests.rs"]
mod tests;
