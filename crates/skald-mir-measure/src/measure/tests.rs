use super::{measure_corpus, MeasurementOptions, REACHABILITY_PASS};
use crate::{
    corpus::{Corpus, Workload, WorkloadKind},
    load_corpus,
    model::MeasurementReport,
    render_report, ReportFormat,
};
use serde_json::Value;
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
        .retain_ids(&BTreeSet::from(["focused/local-simplification".to_owned()]))
        .unwrap();
    let first = measure_corpus(&root, &corpus, MeasurementOptions::default()).unwrap();
    let second = measure_corpus(&root, &corpus, MeasurementOptions::default()).unwrap();
    assert_structural_reports_equal(&first, &second);
    let mut different_invocation_context = first.clone();
    different_invocation_context.compiler.revision = "different-revision".to_owned();
    different_invocation_context.compiler.dirty = !different_invocation_context.compiler.dirty;
    assert_structural_reports_equal(&first, &different_invocation_context);
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
    for snapshot in first.workloads()[0].snapshots() {
        let detail_names = snapshot
            .redundant_casts()
            .details
            .iter()
            .map(|detail| detail.name.as_str())
            .collect::<BTreeSet<_>>();
        assert!(detail_names.contains("eliminated-cast-steps-upper-bound"));
        assert!(detail_names.contains("maximum-chain-depth"));
        let activation_detail_names = snapshot
            .dead_path_activations()
            .details
            .iter()
            .map(|detail| detail.name.as_str())
            .collect::<BTreeSet<_>>();
        assert!(activation_detail_names.contains("maximum-protocol-size"));
        assert!(activation_detail_names.contains("removable-loads-upper-bound"));
    }
    assert_eq!(
        first.workloads()[0].snapshots()[2]
            .dead_path_activations()
            .proven(),
        first.totals().snapshots()[2]
            .dead_path_activations()
            .proven()
    );
    assert_eq!(
        first.workloads()[0].snapshots()[2]
            .dead_path_activations()
            .proven(),
        0
    );
    assert_eq!(
        first.workloads()[0].snapshots()[1]
            .dead_path_activations()
            .proven(),
        0
    );
    let human = render_report(&first, ReportFormat::Human).unwrap();
    let json = render_report(&first, ReportFormat::Json).unwrap();
    assert!(human.contains("activations 0/0"));
    assert!(json.contains("\"dead_path_activations\""));
    assert!(json.contains("\"removable_storages_upper_bound\""));
}

fn assert_structural_reports_equal(first: &MeasurementReport, second: &MeasurementReport) {
    // Compiler revision and dirty state describe the repository at the instant
    // each report starts. They are useful evidence, but are not part of the
    // measured MIR projection and may legitimately change between invocations.
    let mut first = serde_json::to_value(first).expect("first report must serialize");
    let mut second = serde_json::to_value(second).expect("second report must serialize");
    remove_compiler_context(&mut first);
    remove_compiler_context(&mut second);

    if let Some(difference) = first_json_difference(&first, &second, "$".to_owned()) {
        panic!("structural measurement reports differ at {difference}");
    }
}

fn remove_compiler_context(report: &mut Value) {
    report
        .as_object_mut()
        .expect("measurement report must serialize as an object")
        .remove("compiler")
        .expect("measurement report must contain compiler context");
}

fn first_json_difference(first: &Value, second: &Value, path: String) -> Option<String> {
    match (first, second) {
        (Value::Object(first), Value::Object(second)) => {
            for (name, first_value) in first {
                let Some(second_value) = second.get(name) else {
                    return Some(format!("{path}.{name}: missing from second report"));
                };
                if let Some(difference) =
                    first_json_difference(first_value, second_value, format!("{path}.{name}"))
                {
                    return Some(difference);
                }
            }
            second
                .keys()
                .find(|name| !first.contains_key(*name))
                .map(|name| format!("{path}.{name}: missing from first report"))
        }
        (Value::Array(first), Value::Array(second)) => {
            for (index, (first_value, second_value)) in first.iter().zip(second.iter()).enumerate()
            {
                if let Some(difference) =
                    first_json_difference(first_value, second_value, format!("{path}[{index}]"))
                {
                    return Some(difference);
                }
            }
            (first.len() != second.len()).then(|| {
                format!(
                    "{path}.length: first report has {}, second report has {}",
                    first.len(),
                    second.len()
                )
            })
        }
        _ if first == second => None,
        _ => Some(format!("{path}: first is {first}, second is {second}")),
    }
}

#[test]
fn report_examples_retain_owned_site_locations_and_classification() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut corpus = load_corpus(
        &root,
        Path::new("tests/measurements/local_mir_redundancy.toml"),
    )
    .unwrap();
    corpus
        .retain_ids(&BTreeSet::from(["focused/checked-protocols".to_owned()]))
        .unwrap();

    let report = measure_corpus(&root, &corpus, MeasurementOptions::default()).unwrap();
    let final_snapshot = report.workloads()[0]
        .snapshots()
        .iter()
        .find(|snapshot| snapshot.name() == "final")
        .unwrap();
    let example = final_snapshot
        .scalar_spill
        .examples
        .first()
        .expect("focused checked protocols retain scalar-spill examples");

    assert!(example.callable.starts_with('f'));
    assert!(example
        .block
        .as_deref()
        .is_some_and(|block| block.contains(":b")));
    assert!(example.instruction.is_some());
    assert!(example
        .value
        .as_deref()
        .is_some_and(|value| value.contains(":v")));
    assert!(matches!(
        example.classification.as_str(),
        "proven" | "blocked"
    ));
    let activation = final_snapshot
        .dead_path_activations
        .examples
        .first()
        .expect("normalized activations retain storage-centered examples");
    assert!(activation
        .storage
        .as_deref()
        .is_some_and(|value| value.contains(":s")));
    assert_eq!(activation.value, None);
}
