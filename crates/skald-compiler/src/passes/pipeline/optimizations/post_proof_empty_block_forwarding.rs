//! Normalized empty-goto-block forwarding.

use std::collections::BTreeMap;

use crate::mir::rewrite::{
    analyze_empty_block_forwarding, final_cfg_facts_for_definition,
    MirEmptyBlockForwardingBarrierKind, MirEmptyBlockForwardingPlan, MirFinalCfgFacts,
};

use super::super::{
    execution::{
        MirFinalPassCapability, MirFinalPassOutcome, MirPassData, MirPassFailure,
        MirPassMeasurement,
    },
    policy::{MirPassDescriptor, MirPassImplementation, MirPassRegistration},
    MirPassIdentity, MirPassStage,
};

pub(in crate::passes::pipeline) const IDENTITY: MirPassIdentity = MirPassIdentity::new(7);
const NAME: &str = "post-proof-empty-block-forwarding";
const DESCRIPTION: &str = "Forwards normalized MIR edges through instruction-free goto blocks.";
const REMOVED_FORWARDING_BLOCKS: &str = "removed forwarding blocks";
const REDIRECTED_SUCCESSOR_OCCURRENCES: &str = "redirected successor occurrences";
const RETAINED_CYCLIC_BLOCKS: &str = "retained cyclic forwarding blocks";
const RETAINED_PERMANENT_ATTACHMENT_BARRIERS: &str = "retained permanent-attachment barriers";

pub(in crate::passes::pipeline) const REGISTRATION: MirPassRegistration = MirPassRegistration::new(
    MirPassDescriptor::new(IDENTITY, MirPassStage::Final, NAME, DESCRIPTION),
    MirPassImplementation::final_stage(IDENTITY, transform),
);

struct CallableForwardingPlan {
    facts: MirFinalCfgFacts,
    plan: MirEmptyBlockForwardingPlan,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct ForwardingCounts {
    removed_blocks: usize,
    redirected_occurrences: usize,
    retained_cyclic_blocks: usize,
    retained_permanent_attachment_barriers: usize,
}

impl ForwardingCounts {
    fn add_analysis(&mut self, facts: &MirFinalCfgFacts) -> MirEmptyBlockForwardingPlan {
        let analysis = analyze_empty_block_forwarding(facts);
        let barriers = analysis.counts();
        let cyclic_blocks = barriers
            .barriers_of_kind(MirEmptyBlockForwardingBarrierKind::SelfLoop)
            .saturating_add(barriers.barriers_of_kind(MirEmptyBlockForwardingBarrierKind::Cycle))
            .saturating_add(
                barriers.barriers_of_kind(MirEmptyBlockForwardingBarrierKind::LeadsToCycle),
            );
        self.retained_cyclic_blocks = self.retained_cyclic_blocks.saturating_add(cyclic_blocks);
        let permanent_attachment_barriers =
            barriers
                .barriers_of_kind(MirEmptyBlockForwardingBarrierKind::PermanentAttachment)
                .saturating_add(barriers.barriers_of_kind(
                    MirEmptyBlockForwardingBarrierKind::IncomingPermanentAttachment,
                ));
        self.retained_permanent_attachment_barriers = self
            .retained_permanent_attachment_barriers
            .saturating_add(permanent_attachment_barriers);
        analysis.plan().clone()
    }
}

fn transform(capability: MirFinalPassCapability) -> Result<MirFinalPassOutcome, MirPassFailure> {
    let mut plans = BTreeMap::new();
    let mut counts = ForwardingCounts::default();

    for definition in capability.verified().program().executable_definitions() {
        let callable = definition.callable();
        let facts = final_cfg_facts_for_definition(definition).map_err(MirPassFailure::Rewrite)?;
        let plan = counts.add_analysis(&facts);
        plans.insert(callable, CallableForwardingPlan { facts, plan });
    }

    let processed_callables = plans.len();
    if plans.values().all(|planned| planned.plan.is_empty()) {
        return capability.unchanged_with(pass_data(processed_callables, 0, counts));
    }

    let mut changed_callables = 0usize;
    let rewritten = capability.rewrite_cfg(|callable, edit| {
        let planned = plans
            .get(&callable)
            .expect("every rewritten executable callable was analyzed before invalidation");
        if planned.plan.is_empty() {
            return Ok(());
        }

        let forwarding = edit.apply_empty_block_forwarding(&planned.facts, &planned.plan)?;
        changed_callables = changed_callables.saturating_add(1);
        counts.removed_blocks = counts
            .removed_blocks
            .saturating_add(forwarding.removed_blocks());
        counts.redirected_occurrences = counts
            .redirected_occurrences
            .saturating_add(forwarding.redirected_edges());
        Ok(())
    })?;

    rewritten.finish(pass_data(0, changed_callables, counts))
}

fn pass_data(
    processed_callables: usize,
    changed_callables: usize,
    counts: ForwardingCounts,
) -> MirPassData {
    let data = if changed_callables == 0 {
        MirPassData::processed(processed_callables)
    } else {
        MirPassData::changed(changed_callables)
    };
    data.with_measurement(MirPassMeasurement::count(
        REMOVED_FORWARDING_BLOCKS,
        count(counts.removed_blocks),
    ))
    .with_measurement(MirPassMeasurement::count(
        REDIRECTED_SUCCESSOR_OCCURRENCES,
        count(counts.redirected_occurrences),
    ))
    .with_measurement(MirPassMeasurement::count(
        RETAINED_CYCLIC_BLOCKS,
        count(counts.retained_cyclic_blocks),
    ))
    .with_measurement(MirPassMeasurement::count(
        RETAINED_PERMANENT_ATTACHMENT_BARRIERS,
        count(counts.retained_permanent_attachment_barriers),
    ))
}

fn count(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

#[cfg(test)]
#[path = "post_proof_empty_block_forwarding/tests.rs"]
mod tests;
