use crate::test_support::lower_source_to_final_mir;

use super::{MirProofSnapshotAnalysis, MirSnapshotAnalysisUsage};

#[test]
fn usage_aggregation_saturates() {
    let mut usage = MirSnapshotAnalysisUsage::for_test(
        u64::MAX,
        u64::MAX,
        u64::MAX,
        u64::MAX,
        u64::MAX,
        u64::MAX,
    );
    usage.accumulate(MirSnapshotAnalysisUsage::for_test(1, 1, 1, 1, 1, 1));
    assert_eq!(usage.requests(), u64::MAX);
    assert_eq!(usage.computations(), u64::MAX);
    assert_eq!(usage.repeated_snapshot_requests(), u64::MAX);
    assert_eq!(usage.results_before(), u64::MAX);
    assert_eq!(usage.results_inserted(), u64::MAX);
    assert_eq!(usage.results_discarded(), u64::MAX);
}

#[test]
fn resetting_a_snapshot_starts_fresh_repetition_accounting() {
    let program = lower_source_to_final_mir("fn main() -> i64 { return 1 + 2; }");
    let callable = program.executable_definitions().next().unwrap().callable();
    let mut analyses = MirProofSnapshotAnalysis::default();

    let first = analyses.checkpoint();
    analyses.local_constants(&program, callable).unwrap();
    analyses.local_constants(&program, callable).unwrap();
    let first = analyses.usage_since(first);
    assert_eq!(first.requests(), 2);
    assert_eq!(first.repeated_snapshot_requests(), 1);

    analyses.reset();
    let second = analyses.checkpoint();
    analyses.local_constants(&program, callable).unwrap();
    let second = analyses.usage_since(second);
    assert_eq!(second.requests(), 1);
    assert_eq!(second.repeated_snapshot_requests(), 0);
    assert_eq!(second.results_before(), 0);
    assert_eq!(second.results_inserted(), 0);
    assert_eq!(second.results_discarded(), 0);
}
