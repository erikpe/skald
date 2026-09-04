//! Normalized executable-entry-unreachable block elimination.

use std::collections::BTreeSet;

use crate::mir::rewrite::{final_cfg_facts_for_definition, MirLocalCfgFacts};

use super::super::{
    execution::{
        MirFinalPassCapability, MirFinalPassOutcome, MirPassData, MirPassFailure,
        MirPassMeasurement,
    },
    policy::{MirPassDescriptor, MirPassImplementation, MirPassRegistration},
    MirPassIdentity, MirPassStage,
};

pub(in crate::passes::pipeline) const IDENTITY: MirPassIdentity = MirPassIdentity::new(6);
const NAME: &str = "post-proof-unreachable-block-elimination";
const DESCRIPTION: &str =
    "Removes normalized MIR blocks unreachable from executable and permanent roots.";
const REMOVED_BLOCKS: &str = "removed blocks";
const REMOVED_VALUES: &str = "removed value declarations";
const RETAINED_PERMANENT_ROOTS: &str = "retained permanent unreachable roots";

pub(in crate::passes::pipeline) const REGISTRATION: MirPassRegistration = MirPassRegistration::new(
    MirPassDescriptor::new(IDENTITY, MirPassStage::Final, NAME, DESCRIPTION),
    MirPassImplementation::final_stage(IDENTITY, transform),
);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct EliminationCounts {
    removed_blocks: usize,
    removed_values: usize,
    retained_permanent_roots: usize,
}

impl EliminationCounts {
    fn add(&mut self, other: Self) {
        self.removed_blocks = self.removed_blocks.saturating_add(other.removed_blocks);
        self.removed_values = self.removed_values.saturating_add(other.removed_values);
        self.retained_permanent_roots = self
            .retained_permanent_roots
            .saturating_add(other.retained_permanent_roots);
    }
}

fn transform(capability: MirFinalPassCapability) -> Result<MirFinalPassOutcome, MirPassFailure> {
    let mut processed_callables = 0usize;
    let mut unchanged_counts = EliminationCounts::default();
    let mut has_candidate = false;

    for definition in capability.verified().program().executable_definitions() {
        processed_callables = processed_callables.saturating_add(1);
        let facts = final_cfg_facts_for_definition(definition).map_err(MirPassFailure::Rewrite)?;
        has_candidate |= !facts.unreachable().is_empty();
        unchanged_counts.retained_permanent_roots = unchanged_counts
            .retained_permanent_roots
            .saturating_add(retained_permanent_unreachable_roots(&facts));
    }

    if !has_candidate {
        return capability.unchanged_with(pass_data(processed_callables, 0, unchanged_counts));
    }

    let mut changed_callables = 0usize;
    let mut counts = EliminationCounts::default();
    let rewritten = capability.rewrite_cfg(|_, edit| {
        let facts = edit.facts()?;
        let mut callable_counts = EliminationCounts {
            retained_permanent_roots: retained_permanent_unreachable_roots(&facts),
            ..EliminationCounts::default()
        };
        let removal = edit.remove_unreachable_blocks(&facts)?;
        callable_counts.removed_blocks = removal.blocks();
        callable_counts.removed_values = removal.values();
        if removal.blocks() != 0 {
            changed_callables = changed_callables.saturating_add(1);
        }
        counts.add(callable_counts);
        Ok(())
    })?;

    rewritten.finish(pass_data(0, changed_callables, counts))
}

fn retained_permanent_unreachable_roots(facts: &MirLocalCfgFacts) -> usize {
    let entry_reachable = facts
        .entry_reachable()
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    facts
        .protected_roots()
        .iter()
        .filter(|root| !entry_reachable.contains(&root.block()))
        .count()
}

fn pass_data(
    processed_callables: usize,
    changed_callables: usize,
    counts: EliminationCounts,
) -> MirPassData {
    let data = if changed_callables == 0 {
        MirPassData::processed(processed_callables)
    } else {
        MirPassData::changed(changed_callables)
    };
    data.with_measurement(MirPassMeasurement::count(
        REMOVED_BLOCKS,
        count(counts.removed_blocks),
    ))
    .with_measurement(MirPassMeasurement::count(
        REMOVED_VALUES,
        count(counts.removed_values),
    ))
    .with_measurement(MirPassMeasurement::count(
        RETAINED_PERMANENT_ROOTS,
        count(counts.retained_permanent_roots),
    ))
}

fn count(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

#[cfg(test)]
#[path = "post_proof_unreachable_block_elimination/tests.rs"]
mod tests;
