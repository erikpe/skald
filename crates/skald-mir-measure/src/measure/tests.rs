use super::{measure_corpus, MeasurementOptions, REACHABILITY_PASS};
use crate::{
    corpus::{Corpus, Workload, WorkloadKind},
    load_corpus,
};
use std::{collections::BTreeSet, fs, path::Path};

#[test]
fn empty_corpus_produces_a_valid_empty_report() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let corpus = Corpus {
        name: "empty".to_owned(),
        version: 1,
        workloads: Vec::new(),
    };
    let report = measure_corpus(root, &corpus, MeasurementOptions::default()).unwrap();
    assert!(report.workloads().is_empty());
    assert!(report.totals().snapshots().is_empty());
}

#[test]
fn compilation_failure_names_the_owning_workload() {
    let root =
        std::env::temp_dir().join(format!("skald-mir-measure-failure-{}", std::process::id()));
    fs::create_dir_all(root.join("std")).unwrap();
    fs::write(root.join("invalid.ska"), "this is not Skald\n").unwrap();
    let corpus = Corpus {
        name: "failure".to_owned(),
        version: 1,
        workloads: vec![Workload {
            id: "focused/failure".to_owned(),
            category: "focused".to_owned(),
            kind: WorkloadKind::Explicit,
            identity: "invalid".to_owned(),
            entry: root.join("invalid.ska"),
            entry_relative: "invalid.ska".to_owned(),
            native_runs: Vec::new(),
        }],
    };
    let error = measure_corpus(&root, &corpus, MeasurementOptions::default()).unwrap_err();
    assert!(error.to_string().contains("workload \"focused/failure\""));
    assert!(error.to_string().contains("compilation failed"));
    let _ = fs::remove_dir_all(root);
}

#[test]
fn focused_real_driver_measurement_is_deterministic_and_has_semantic_checkpoints() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut corpus = load_corpus(
        &root,
        Path::new("tests/measurements/local_mir_redundancy.toml"),
    )
    .unwrap();
    corpus
        .retain_ids(&BTreeSet::from(["benchmark/range-i64".to_owned()]))
        .unwrap();
    let first = measure_corpus(&root, &corpus, MeasurementOptions::default()).unwrap();
    let second = measure_corpus(&root, &corpus, MeasurementOptions::default()).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.workloads().len(), 1);
    assert_eq!(
        first.workloads()[0]
            .snapshots()
            .iter()
            .map(|snapshot| snapshot.name())
            .collect::<Vec<_>>(),
        ["input", "pre-reachability", "final"]
    );
    assert_eq!(first.schedule.last().unwrap().pass, REACHABILITY_PASS);
    assert_eq!(
        first.workloads()[0].snapshots()[2].scalar_spill().proven(),
        first.totals().snapshots()[2].scalar_spill().proven()
    );
}
