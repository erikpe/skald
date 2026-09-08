//! Saturating deterministic aggregation for dead path-activation observations.

use std::collections::BTreeMap;

use crate::mir::{BlockId, StorageId};

use super::super::{
    count::RedundancyCount,
    path_activation_model::{
        DeadPathActivationBlocker, DeadPathActivationCandidate, DeadPathActivationCounts,
        DeadPathActivationInstructionKind,
    },
    site::{merge_storage_examples, RedundancySiteClassification, RedundancyStorageExample},
};

#[derive(Clone, Debug, Default)]
pub(super) struct Accumulator {
    pub(super) counts: DeadPathActivationCounts,
    primary_blockers: BTreeMap<DeadPathActivationBlocker, u64>,
    barriers: BTreeMap<DeadPathActivationBlocker, u64>,
    pub(super) examples: Vec<RedundancyStorageExample<DeadPathActivationBlocker>>,
}

impl Accumulator {
    pub(super) fn increment_inspected(&mut self) {
        add(&mut self.counts.inspected, 1, &mut self.counts.saturated);
        add(&mut self.counts.interesting, 1, &mut self.counts.saturated);
    }

    pub(super) fn record_candidate(
        &mut self,
        candidate: &DeadPathActivationCandidate,
        location: Option<(BlockId, usize)>,
    ) {
        add(&mut self.counts.proven, 1, &mut self.counts.saturated);
        add(
            &mut self.counts.supporting_values,
            candidate.removable_values_upper_bound(),
            &mut self.counts.saturated,
        );
        add(
            &mut self.counts.supporting_instructions,
            candidate.removable_instructions_upper_bound(),
            &mut self.counts.saturated,
        );
        add(
            &mut self.counts.removable_storages_upper_bound,
            candidate.removable_storages_upper_bound(),
            &mut self.counts.saturated,
        );
        add(
            &mut self.counts.removable_values_upper_bound,
            candidate.removable_values_upper_bound(),
            &mut self.counts.saturated,
        );
        add(
            &mut self.counts.removable_instructions_upper_bound,
            candidate.removable_instructions_upper_bound(),
            &mut self.counts.saturated,
        );
        let mut loads = 0;
        let mut stores = 0;
        let mut lifetimes = 0;
        for instruction in candidate.instructions() {
            match instruction.kind() {
                DeadPathActivationInstructionKind::Load => loads += 1,
                DeadPathActivationInstructionKind::Store => stores += 1,
                DeadPathActivationInstructionKind::LifetimeLive
                | DeadPathActivationInstructionKind::LifetimeDead => lifetimes += 1,
            }
        }
        add(
            &mut self.counts.removable_loads_upper_bound,
            loads,
            &mut self.counts.saturated,
        );
        add(
            &mut self.counts.removable_stores_upper_bound,
            stores,
            &mut self.counts.saturated,
        );
        add(
            &mut self.counts.removable_lifetime_markers_upper_bound,
            lifetimes,
            &mut self.counts.saturated,
        );
        self.counts.maximum_protocol_size = self
            .counts
            .maximum_protocol_size
            .max(candidate.removable_instructions_upper_bound());
        merge_storage_examples(
            &mut self.examples,
            &[RedundancyStorageExample::new(
                candidate.storage().callable(),
                candidate.storage(),
                location,
                RedundancySiteClassification::Proven,
                Vec::new(),
            )],
        );
    }

    pub(super) fn record_blocked(
        &mut self,
        storage: StorageId,
        location: Option<(BlockId, usize)>,
        blockers: &[DeadPathActivationBlocker],
    ) {
        add(&mut self.counts.blocked, 1, &mut self.counts.saturated);
        if let Some(primary) = blockers.first() {
            increment_map(
                &mut self.primary_blockers,
                *primary,
                &mut self.counts.saturated,
            );
        }
        for blocker in blockers {
            increment_map(&mut self.barriers, *blocker, &mut self.counts.saturated);
        }
        merge_storage_examples(
            &mut self.examples,
            &[RedundancyStorageExample::new(
                storage.callable(),
                storage,
                location,
                RedundancySiteClassification::Blocked,
                blockers.to_vec(),
            )],
        );
    }

    pub(super) fn merge(&mut self, other: &Self) {
        macro_rules! add_count {
            ($field:ident) => {
                add(
                    &mut self.counts.$field,
                    other.counts.$field,
                    &mut self.counts.saturated,
                )
            };
        }
        add_count!(inspected);
        add_count!(interesting);
        add_count!(proven);
        add_count!(blocked);
        add_count!(non_candidates);
        add_count!(supporting_values);
        add_count!(supporting_instructions);
        add_count!(removable_storages_upper_bound);
        add_count!(removable_values_upper_bound);
        add_count!(removable_instructions_upper_bound);
        add_count!(removable_loads_upper_bound);
        add_count!(removable_stores_upper_bound);
        add_count!(removable_lifetime_markers_upper_bound);
        self.counts.maximum_protocol_size = self
            .counts
            .maximum_protocol_size
            .max(other.counts.maximum_protocol_size);
        merge_map(
            &mut self.primary_blockers,
            &other.primary_blockers,
            &mut self.counts.saturated,
        );
        merge_map(
            &mut self.barriers,
            &other.barriers,
            &mut self.counts.saturated,
        );
        merge_storage_examples(&mut self.examples, &other.examples);
        self.counts.saturated |= other.counts.saturated;
    }

    pub(super) fn finish(mut self, affected_callables: u64) -> DeadPathActivationCounts {
        self.counts.affected_callables = affected_callables;
        self.counts.primary_blockers = finish_map(self.primary_blockers);
        self.counts.barriers = finish_map(self.barriers);
        self.counts
    }
}

fn add(target: &mut u64, amount: u64, saturated: &mut bool) {
    match target.checked_add(amount) {
        Some(value) => *target = value,
        None => {
            *target = u64::MAX;
            *saturated = true;
        }
    }
}

fn increment_map(
    counts: &mut BTreeMap<DeadPathActivationBlocker, u64>,
    key: DeadPathActivationBlocker,
    saturated: &mut bool,
) {
    add(counts.entry(key).or_default(), 1, saturated);
}

fn merge_map(
    target: &mut BTreeMap<DeadPathActivationBlocker, u64>,
    source: &BTreeMap<DeadPathActivationBlocker, u64>,
    saturated: &mut bool,
) {
    for (key, count) in source {
        add(target.entry(*key).or_default(), *count, saturated);
    }
}

fn finish_map(
    counts: BTreeMap<DeadPathActivationBlocker, u64>,
) -> Vec<RedundancyCount<DeadPathActivationBlocker>> {
    counts
        .into_iter()
        .map(|(key, sites)| RedundancyCount::new(key, sites))
        .collect()
}
