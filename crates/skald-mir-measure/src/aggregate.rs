//! Saturating deterministic aggregation of workload snapshots.

use std::collections::{BTreeMap, BTreeSet};

use crate::model::{
    CandidateCounts, CategoryCoverage, NamedCount, SnapshotReport, StructureCounts, Totals,
    WorkloadReport,
};

pub(super) fn totals(workloads: &[WorkloadReport]) -> Totals {
    let mut snapshots = BTreeMap::<String, SnapshotReport>::new();
    let mut categories = BTreeMap::<String, BTreeSet<String>>::new();
    for workload in workloads {
        let has_proven = workload
            .snapshots
            .iter()
            .find(|snapshot| snapshot.name == "final")
            .is_some_and(|snapshot| {
                snapshot.scalar_spill.proven > 0
                    || snapshot.redundant_casts.proven > 0
                    || snapshot.local_cse.proven > 0
            });
        if has_proven {
            categories
                .entry(workload.category.clone())
                .or_default()
                .insert(workload.id.clone());
        } else {
            categories.entry(workload.category.clone()).or_default();
        }
        for snapshot in &workload.snapshots {
            let aggregate = snapshots
                .entry(snapshot.name.clone())
                .or_insert_with(|| empty_snapshot(&snapshot.name));
            merge_snapshot(aggregate, snapshot);
        }
    }
    let snapshots = ["input", "pre-reachability", "final"]
        .into_iter()
        .filter_map(|name| snapshots.remove(name))
        .collect::<Vec<_>>();
    let workload_categories = categories
        .into_iter()
        .map(|(category, workload_ids)| CategoryCoverage {
            category,
            workloads_with_proven_candidates: workload_ids.into_iter().collect(),
        })
        .collect();
    let saturated = snapshots.iter().any(|snapshot| snapshot.saturated);
    Totals {
        snapshots,
        workload_categories,
        saturated,
    }
}

fn empty_snapshot(name: &str) -> SnapshotReport {
    SnapshotReport {
        name: name.to_owned(),
        ..SnapshotReport::default()
    }
}

fn merge_snapshot(target: &mut SnapshotReport, source: &SnapshotReport) {
    merge_structure(&mut target.structure, &source.structure);
    merge_candidate(&mut target.scalar_spill, &source.scalar_spill);
    merge_candidate(&mut target.redundant_casts, &source.redundant_casts);
    merge_candidate(&mut target.local_cse, &source.local_cse);
    target.overlaps = merge_named_pairs(&target.overlaps, &source.overlaps, &mut target.saturated);
    target.saturated = target.structure.saturated
        || target.scalar_spill.saturated
        || target.redundant_casts.saturated
        || target.local_cse.saturated
        || source.saturated;
}

fn merge_structure(target: &mut StructureCounts, source: &StructureCounts) {
    add(
        &mut target.definitions,
        source.definitions,
        &mut target.saturated,
    );
    add(
        &mut target.executable_definitions,
        source.executable_definitions,
        &mut target.saturated,
    );
    add(&mut target.blocks, source.blocks, &mut target.saturated);
    add(
        &mut target.instructions,
        source.instructions,
        &mut target.saturated,
    );
    add(&mut target.values, source.values, &mut target.saturated);
    add(&mut target.storages, source.storages, &mut target.saturated);
    target.saturated |= source.saturated;
}

fn merge_candidate(target: &mut CandidateCounts, source: &CandidateCounts) {
    macro_rules! add_field {
        ($field:ident) => {
            add(&mut target.$field, source.$field, &mut target.saturated)
        };
    }
    add_field!(inspected);
    add_field!(interesting);
    add_field!(proven);
    add_field!(blocked);
    add_field!(non_candidates);
    add_field!(affected_callables);
    add_field!(supporting_values);
    add_field!(supporting_instructions);
    add_field!(removable_values_upper_bound);
    add_field!(removable_instructions_upper_bound);
    target.outcomes = merge_named(&target.outcomes, &source.outcomes, &mut target.saturated);
    target.primary_blockers = merge_named(
        &target.primary_blockers,
        &source.primary_blockers,
        &mut target.saturated,
    );
    target.barriers = merge_named(&target.barriers, &source.barriers, &mut target.saturated);
    target.consumers = merge_named(&target.consumers, &source.consumers, &mut target.saturated);
    target.unlocks = merge_named(&target.unlocks, &source.unlocks, &mut target.saturated);
    target.details = merge_named(&target.details, &source.details, &mut target.saturated);
    target.saturated |= source.saturated;
}

fn merge_named(left: &[NamedCount], right: &[NamedCount], saturated: &mut bool) -> Vec<NamedCount> {
    let mut counts = BTreeMap::<String, u64>::new();
    for count in left.iter().chain(right) {
        let target = counts.entry(count.name.clone()).or_default();
        add(target, count.sites, saturated);
    }
    counts
        .into_iter()
        .map(|(name, sites)| NamedCount { name, sites })
        .collect()
}

fn merge_named_pairs(
    left: &[crate::model::OverlapCount],
    right: &[crate::model::OverlapCount],
    saturated: &mut bool,
) -> Vec<crate::model::OverlapCount> {
    let mut counts = BTreeMap::<(&'static str, String), u64>::new();
    for count in left.iter().chain(right) {
        let entry = counts
            .entry((count.enabler, count.consumer.clone()))
            .or_default();
        add(entry, count.sites, saturated);
    }
    counts
        .into_iter()
        .map(|((enabler, consumer), sites)| crate::model::OverlapCount {
            enabler,
            consumer,
            sites,
        })
        .collect()
}

pub(super) fn add(target: &mut u64, value: u64, saturated: &mut bool) {
    match target.checked_add(value) {
        Some(sum) => *target = sum,
        None => {
            *target = u64::MAX;
            *saturated = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{add, merge_candidate};
    use crate::model::{CandidateCounts, NamedCount};

    #[test]
    fn saturation_is_sticky_and_explicit() {
        let mut value = u64::MAX - 1;
        let mut saturated = false;
        add(&mut value, 2, &mut saturated);
        assert_eq!(value, u64::MAX);
        assert!(saturated);
        add(&mut value, 0, &mut saturated);
        assert!(saturated);
    }

    #[test]
    fn candidate_aggregation_preserves_sorted_detail_and_saturation() {
        let mut target = CandidateCounts {
            inspected: u64::MAX,
            outcomes: vec![NamedCount {
                name: "zeta".to_owned(),
                sites: 1,
            }],
            ..CandidateCounts::default()
        };
        let source = CandidateCounts {
            inspected: 1,
            outcomes: vec![NamedCount {
                name: "alpha".to_owned(),
                sites: 2,
            }],
            ..CandidateCounts::default()
        };
        merge_candidate(&mut target, &source);
        assert_eq!(target.inspected, u64::MAX);
        assert!(target.saturated);
        assert_eq!(
            target
                .outcomes
                .iter()
                .map(|count| count.name.as_str())
                .collect::<Vec<_>>(),
            ["alpha", "zeta"]
        );
    }
}
