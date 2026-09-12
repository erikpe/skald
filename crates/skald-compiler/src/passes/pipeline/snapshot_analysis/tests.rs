use crate::test_support::lower_source_to_final_mir;

use std::sync::Arc;

use crate::{
    identity::{CallableId, FunctionId},
    mir::ValueId,
};

use super::{MirProofSnapshotAnalysis, MirSnapshotAnalysisPolicy, MirSnapshotAnalysisUsage};

#[test]
fn usage_aggregation_saturates() {
    let mut usage = MirSnapshotAnalysisUsage::for_test(
        u64::MAX,
        u64::MAX,
        u64::MAX,
        u64::MAX,
        u64::MAX,
        u64::MAX,
        u64::MAX,
    );
    usage.accumulate(MirSnapshotAnalysisUsage::for_test(1, 1, 1, 1, 1, 1, 1));
    assert_eq!(usage.requests(), u64::MAX);
    assert_eq!(usage.computations(), u64::MAX);
    assert_eq!(usage.hits(), u64::MAX);
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

#[test]
fn memoized_sessions_share_successful_results_until_reset() {
    let program = lower_source_to_final_mir("fn main() -> i64 { return 1 + 2; }");
    let callable = program.executable_definitions().next().unwrap().callable();
    let mut analyses = MirProofSnapshotAnalysis::new(MirSnapshotAnalysisPolicy::Memoized);

    let checkpoint = analyses.checkpoint();
    let first = analyses.local_constants(&program, callable).unwrap();
    let second = analyses.local_constants(&program, callable).unwrap();
    assert!(Arc::ptr_eq(&first, &second));
    let usage = analyses.usage_since(checkpoint);
    assert_eq!(usage.requests(), 2);
    assert_eq!(usage.computations(), 1);
    assert_eq!(usage.hits(), 1);
    assert_eq!(usage.results_inserted(), 1);

    let checkpoint = analyses.checkpoint();
    analyses.reset();
    let third = analyses.local_constants(&program, callable).unwrap();
    assert!(!Arc::ptr_eq(&first, &third));
    let usage = analyses.usage_since(checkpoint);
    assert_eq!(usage.computations(), 1);
    assert_eq!(usage.results_before(), 1);
    assert_eq!(usage.results_inserted(), 1);
    assert_eq!(usage.results_discarded(), 1);
}

#[test]
fn sessions_reject_callables_absent_from_the_exact_program() {
    let program = lower_source_to_final_mir("fn main() -> i64 { return 0; }");
    let callable = CallableId::Function(FunctionId::new(usize::MAX));
    let mut analyses = MirProofSnapshotAnalysis::new(MirSnapshotAnalysisPolicy::Memoized);

    let checkpoint = analyses.checkpoint();
    let error = analyses.local_constants(&program, callable).unwrap_err();
    assert_eq!(
        error,
        crate::passes::pipeline::optimizations::LocalConstantAnalysisError::UnknownExecutableCallable {
            callable,
        }
    );
    let usage = analyses.usage_since(checkpoint);
    assert_eq!(usage, MirSnapshotAnalysisUsage::default());
}

#[test]
fn memoized_sessions_never_insert_failed_computations() {
    let mut program = lower_source_to_final_mir("fn main() -> i64 { return 1; }");
    let callable = CallableId::Function(program.entry_function);
    let definition = program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap();
    let expected = definition.values[0].id;
    definition.values[0].id = ValueId::new(
        CallableId::Function(FunctionId::new(usize::MAX)),
        expected.index(),
    );
    let mut analyses = MirProofSnapshotAnalysis::new(MirSnapshotAnalysisPolicy::Memoized);

    let checkpoint = analyses.checkpoint();
    assert!(analyses.local_constants(&program, callable).is_err());
    assert!(analyses.local_constants(&program, callable).is_err());
    let usage = analyses.usage_since(checkpoint);
    assert_eq!(
        (usage.requests(), usage.computations(), usage.hits()),
        (2, 2, 0)
    );
    assert_eq!(usage.results_inserted(), 0);
}
