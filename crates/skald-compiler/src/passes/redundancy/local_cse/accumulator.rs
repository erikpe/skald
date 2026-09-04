//! Deterministic aggregate accounting for the local CSE census.

use std::collections::BTreeSet;

use crate::{
    identity::CallableId,
    mir::{
        rewrite::{value_use_sites_for_definition, MirRewriteError},
        MirDefinitionRef, ValueId,
    },
};

use super::super::site::{merge_examples, RedundancySiteClassification, RedundancySiteExample};
use super::{
    add_use_barriers, consumer, validation_barriers, LocalCseBlocker, LocalCseConsumer,
    LocalCseCount, LocalCseExcludedFamily, LocalCseObservationCounts, LocalCseOperationFamily,
    LocalCseOutcome, Site,
};

#[derive(Default)]
pub(super) struct Accumulator {
    pub(super) counts: LocalCseObservationCounts,
    supporting_values: BTreeSet<ValueId>,
    supporting_instructions: BTreeSet<(CallableId, usize, usize)>,
    pub(super) examples: Vec<RedundancySiteExample<LocalCseBlocker>>,
}

impl Accumulator {
    pub(super) fn has_observations(&self) -> bool {
        self.counts.inspected != 0 || !self.counts.excluded_families.is_empty()
    }

    pub(super) fn increment_inspected(&mut self) {
        add(&mut self.counts.inspected, 1, &mut self.counts.saturated);
    }

    pub(super) fn increment_non_candidate(&mut self) {
        add(
            &mut self.counts.non_candidates,
            1,
            &mut self.counts.saturated,
        );
    }

    pub(super) fn increment_operation_family(&mut self, key: LocalCseOperationFamily) {
        increment(
            &mut self.counts.operation_families,
            key,
            &mut self.counts.saturated,
        );
    }

    pub(super) fn increment_excluded(&mut self, key: LocalCseExcludedFamily) {
        increment(
            &mut self.counts.excluded_families,
            key,
            &mut self.counts.saturated,
        );
    }

    pub(super) fn increment_scalar_spill_unlock(&mut self) {
        add(
            &mut self.counts.scalar_spill_unlocks,
            1,
            &mut self.counts.saturated,
        );
    }

    pub(super) fn maximum_repetitions(&mut self, repetitions: u64) {
        self.counts.maximum_repetitions_per_key =
            self.counts.maximum_repetitions_per_key.max(repetitions);
    }

    pub(super) fn record_candidate(
        &mut self,
        definition: MirDefinitionRef<'_>,
        first: Site,
        repeated: Site,
        malformed_values: bool,
    ) -> Result<(), MirRewriteError> {
        add(&mut self.counts.interesting, 1, &mut self.counts.saturated);
        self.supporting_values.insert(first.result);
        self.supporting_values.insert(repeated.result);
        self.supporting_instructions
            .insert((first.callable, first.block, first.instruction));
        self.supporting_instructions.insert((
            repeated.callable,
            repeated.block,
            repeated.instruction,
        ));

        let mut barriers = validation_barriers(definition, first, malformed_values);
        barriers.extend(validation_barriers(definition, repeated, malformed_values));
        let uses = match value_use_sites_for_definition(definition, repeated.result) {
            Ok(uses) => uses,
            Err(_) if barriers.contains(&LocalCseBlocker::MalformedIdentity) => {
                increment(
                    &mut self.counts.consumers,
                    LocalCseConsumer::Other,
                    &mut self.counts.saturated,
                );
                return self.finish_candidate(repeated, barriers, LocalCseOutcome::Replaceable, 0);
            }
            Err(error) => return Err(error),
        };
        let outcome = if uses.uses().is_empty() {
            increment(
                &mut self.counts.consumers,
                LocalCseConsumer::Dead,
                &mut self.counts.saturated,
            );
            LocalCseOutcome::DeadResult
        } else {
            for use_site in uses.uses() {
                increment(
                    &mut self.counts.consumers,
                    consumer(use_site.role()),
                    &mut self.counts.saturated,
                );
            }
            add_use_barriers(&mut barriers, &uses, repeated.block);
            LocalCseOutcome::Replaceable
        };
        self.finish_candidate(repeated, barriers, outcome, uses.uses().len() as u64)
    }

    fn finish_candidate(
        &mut self,
        site: Site,
        barriers: BTreeSet<LocalCseBlocker>,
        outcome: LocalCseOutcome,
        replaceable_uses: u64,
    ) -> Result<(), MirRewriteError> {
        increment(
            &mut self.counts.outcomes,
            outcome,
            &mut self.counts.saturated,
        );
        for barrier in barriers.iter().copied() {
            increment(
                &mut self.counts.barriers,
                barrier,
                &mut self.counts.saturated,
            );
        }
        let classification = if barriers.is_empty() {
            add(&mut self.counts.proven, 1, &mut self.counts.saturated);
            add(
                &mut self.counts.replaceable_uses,
                replaceable_uses,
                &mut self.counts.saturated,
            );
            add(
                &mut self.counts.removable_values_upper_bound,
                1,
                &mut self.counts.saturated,
            );
            add(
                &mut self.counts.removable_instructions_upper_bound,
                1,
                &mut self.counts.saturated,
            );
            RedundancySiteClassification::Proven
        } else {
            add(&mut self.counts.blocked, 1, &mut self.counts.saturated);
            increment(
                &mut self.counts.primary_blockers,
                *barriers.iter().next().unwrap(),
                &mut self.counts.saturated,
            );
            RedundancySiteClassification::Blocked
        };
        merge_examples(
            &mut self.examples,
            &[RedundancySiteExample::new(
                site.callable,
                site.block_id,
                site.instruction,
                Some(site.result),
                classification,
                barriers.into_iter().collect(),
            )],
        );
        Ok(())
    }

    pub(super) fn merge(&mut self, other: &Self) {
        macro_rules! merge_field {
            ($field:ident) => {
                add(
                    &mut self.counts.$field,
                    other.counts.$field,
                    &mut self.counts.saturated,
                );
            };
        }
        merge_field!(inspected);
        merge_field!(interesting);
        merge_field!(proven);
        merge_field!(blocked);
        merge_field!(non_candidates);
        merge_field!(removable_values_upper_bound);
        merge_field!(removable_instructions_upper_bound);
        merge_field!(replaceable_uses);
        merge_field!(scalar_spill_unlocks);
        self.counts.maximum_repetitions_per_key = self
            .counts
            .maximum_repetitions_per_key
            .max(other.counts.maximum_repetitions_per_key);
        merge_counts(
            &mut self.counts.outcomes,
            &other.counts.outcomes,
            &mut self.counts.saturated,
        );
        merge_counts(
            &mut self.counts.operation_families,
            &other.counts.operation_families,
            &mut self.counts.saturated,
        );
        merge_counts(
            &mut self.counts.primary_blockers,
            &other.counts.primary_blockers,
            &mut self.counts.saturated,
        );
        merge_counts(
            &mut self.counts.barriers,
            &other.counts.barriers,
            &mut self.counts.saturated,
        );
        merge_counts(
            &mut self.counts.consumers,
            &other.counts.consumers,
            &mut self.counts.saturated,
        );
        merge_counts(
            &mut self.counts.excluded_families,
            &other.counts.excluded_families,
            &mut self.counts.saturated,
        );
        self.supporting_values
            .extend(other.supporting_values.iter().copied());
        self.supporting_instructions
            .extend(other.supporting_instructions.iter().copied());
        merge_examples(&mut self.examples, &other.examples);
        self.counts.saturated |= other.counts.saturated;
    }

    pub(super) fn finish(mut self, affected_callables: u64) -> LocalCseObservationCounts {
        self.counts.affected_callables = affected_callables;
        self.counts.supporting_values = self.supporting_values.len() as u64;
        self.counts.supporting_instructions = self.supporting_instructions.len() as u64;
        self.counts
    }
}

fn add(total: &mut u64, value: u64, saturated: &mut bool) {
    let (sum, overflow) = total.overflowing_add(value);
    *total = if overflow { u64::MAX } else { sum };
    *saturated |= overflow;
}

fn increment<T: Copy + Eq>(counts: &mut Vec<LocalCseCount<T>>, key: T, saturated: &mut bool) {
    if let Some(count) = counts.iter_mut().find(|count| count.key == key) {
        add(&mut count.sites, 1, saturated);
    } else {
        counts.push(LocalCseCount::new(key, 1));
    }
}

fn merge_counts<T: Copy + Eq>(
    target: &mut Vec<LocalCseCount<T>>,
    source: &[LocalCseCount<T>],
    saturated: &mut bool,
) {
    for source_count in source {
        if let Some(count) = target
            .iter_mut()
            .find(|count| count.key == source_count.key)
        {
            add(&mut count.sites, source_count.sites, saturated);
        } else {
            target.push(*source_count);
        }
    }
}
