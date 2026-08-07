mod support;

use skald_golden::{
    execute_parallel, execute_sequential, select, Determinism, ProcessTermination,
    SchedulerOptions, SelectionOptions, StageStatus,
};
use std::{collections::BTreeSet, fs, num::NonZeroUsize};
use support::{write_compile_fail_spec, write_native_spec, Fixture};

fn scheduler(jobs: usize, fail_fast: bool) -> SchedulerOptions {
    SchedulerOptions::new(NonZeroUsize::new(jobs).unwrap()).with_fail_fast(fail_fast)
}

fn read_count(path: &std::path::Path) -> u64 {
    fs::read_to_string(path).unwrap().parse().unwrap()
}

fn write_compile_fail_matrix(fixture: &Fixture, count: usize) {
    let mut spec = String::from("schema=1\n");
    for index in 0..count {
        fixture.write(
            &format!("failure-{index}.ska"),
            "fn main() -> i64 { return missing(); }\n",
        );
        spec.push_str(&format!(
            r#"
[[test]]
name="failure-{index}"
mode="compile-fail"
source="failure-{index}.ska"
compiler_args=["--fake-mode","compile-fail"]
expect={{stderr={{match="contains",inline="error[FAKE001]"}}}}
"#
        ));
    }
    fixture.write("matrix.golden.toml", spec);
}

#[test]
fn bounds_external_processes_and_compiles_independent_sources_concurrently() {
    let fixture = Fixture::new();
    write_compile_fail_matrix(&fixture, 7);
    let (active, peak) = fixture.activity_paths("global");
    let plan = fixture.plan();
    let selected = select(&plan, &SelectionOptions::default()).unwrap();
    let options =
        fixture.options_with_activity(Determinism::Off, "success", Some((&active, &peak, 60)));

    let execution = execute_parallel(&selected, &options, scheduler(3, false));

    assert!(execution.passed(), "{execution:#?}");
    assert_eq!(read_count(&active), 0);
    assert_eq!(read_count(&peak), 3);
}

#[test]
fn returns_canonical_results_across_varied_completion_orders() {
    let mut random_state = 0x9e37_79b9_7f4a_7c15;
    let schedules: [[(&str, u64); 4]; 4] = std::array::from_fn(|_| {
        let mut delays = [0, 40, 80, 120];
        for index in (1..delays.len()).rev() {
            let selected = (next_random(&mut random_state) as usize) % (index + 1);
            delays.swap(index, selected);
        }
        [
            ("a", delays[0]),
            ("b", delays[1]),
            ("c", delays[2]),
            ("d", delays[3]),
        ]
    });

    let mut observed_orders = BTreeSet::new();
    for schedule in schedules {
        let fixture = Fixture::new();
        let completion_log = fixture.root.join("completion.log");
        let mut spec = String::from("schema=1\n");
        for (name, delay) in schedule {
            fixture.write(
                &format!("{name}.ska"),
                "fn main() -> i64 { return missing(); }\n",
            );
            spec.push_str(&format!(
                r#"
[[test]]
name="{name}"
mode="compile-fail"
source="{name}.ska"
compiler_args=["--fake-mode","compile-fail","--fake-delay-ms","{delay}","--fake-completion-log",{log:?},"--fake-label","{name}"]
expect={{stderr={{match="contains",inline="error[FAKE001]"}}}}
"#,
                log = completion_log.display().to_string(),
            ));
        }
        fixture.write("order.golden.toml", spec);
        let plan = fixture.plan();
        let selected = select(&plan, &SelectionOptions::default()).unwrap();

        let execution = execute_parallel(
            &selected,
            &fixture.options(Determinism::Off, "success"),
            scheduler(4, false),
        );

        assert!(execution.passed(), "{execution:#?}");
        let completion = fs::read_to_string(completion_log).unwrap();
        let mut completed_names = completion.lines().collect::<Vec<_>>();
        completed_names.sort_unstable();
        assert_eq!(completed_names, ["a", "b", "c", "d"]);
        observed_orders.insert(completion);
        let ids = execution
            .leaves()
            .iter()
            .map(|leaf| leaf.leaf_id())
            .collect::<Vec<_>>();
        assert!(ids.windows(2).all(|pair| pair[0] < pair[1]));
    }
    assert!(observed_orders.len() > 1, "schedules completed identically");
}

fn next_random(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

#[test]
fn independent_runs_overlap_but_named_resources_exclude_each_other() {
    for (resources, expected_peak) in [("", 2), ("resources=[\"shared\"]", 1)] {
        let fixture = Fixture::new();
        let (active, peak) = fixture.activity_paths("runs");
        let environment = format!(
            "env={{SKALD_FAKE_ACTIVE={:?},SKALD_FAKE_PEAK={:?},SKALD_FAKE_DELAY_MS=\"60\"}}",
            active.display().to_string(),
            peak.display().to_string()
        );
        write_native_spec(
            &fixture,
            "success",
            &format!(
                "args=[\"sleep\",\"1\"]\n{environment}\n{resources}\n[[test.run]]\nname=\"second\"\nargs=[\"sleep\",\"1\"]\n{environment}\n{resources}"
            ),
        );
        let plan = fixture.plan();
        let selected = select(&plan, &SelectionOptions::default()).unwrap();

        let execution = execute_parallel(
            &selected,
            &fixture.options(Determinism::Off, "success"),
            scheduler(4, false),
        );

        assert!(execution.passed(), "{execution:#?}");
        assert_eq!(read_count(&peak), expected_peak);
    }
}

#[test]
fn serial_nodes_run_without_any_other_active_node() {
    let fixture = Fixture::new();
    let (active, peak) = fixture.activity_paths("serial");
    let environment = format!(
        "env={{SKALD_FAKE_ACTIVE={:?},SKALD_FAKE_PEAK={:?},SKALD_FAKE_DELAY_MS=\"60\"}}",
        active.display().to_string(),
        peak.display().to_string()
    );
    write_native_spec(
        &fixture,
        "success",
        &format!(
            "args=[\"sleep\",\"1\"]\n{environment}\nserial=true\n[[test.run]]\nname=\"second\"\nargs=[\"sleep\",\"1\"]\n{environment}"
        ),
    );
    let plan = fixture.plan();
    let selected = select(&plan, &SelectionOptions::default()).unwrap();

    let execution = execute_parallel(
        &selected,
        &fixture.options(Determinism::Off, "success"),
        scheduler(4, false),
    );

    assert!(execution.passed(), "{execution:#?}");
    assert_eq!(read_count(&peak), 1);
}

#[test]
fn fail_fast_stops_new_unrelated_work_after_the_first_failure() {
    let fixture = Fixture::new();
    fixture.write("a.ska", "fn main() -> i64 { return missing(); }\n");
    fixture.write("z.ska", "fn main() -> i64 { return missing(); }\n");
    fixture.write(
        "fail-fast.golden.toml",
        r#"schema=1
[[test]]
name="a-failure"
mode="compile-fail"
source="a.ska"
compiler_args=["--fake-mode","status-two"]
expect={stderr={inline="unused"}}
[[test]]
name="z-unrelated"
mode="compile-fail"
source="z.ska"
compiler_args=["--fake-mode","compile-fail"]
expect={stderr={match="contains",inline="error[FAKE001]"}}
"#,
    );
    let plan = fixture.plan();
    let selected = select(&plan, &SelectionOptions::default()).unwrap();
    let options = fixture.options(Determinism::Off, "success");

    let execution = execute_parallel(&selected, &options, scheduler(1, true));

    assert!(!execution.passed());
    assert!(matches!(
        execution.leaves()[0].status(),
        StageStatus::Failed(_)
    ));
    assert!(matches!(
        execution.leaves()[1].status(),
        StageStatus::Cancelled { dependency } if dependency == "fail-fast"
    ));
    assert!(execution.builds()[1]
        .compilation()
        .observations()
        .is_empty());
}

#[test]
fn prerequisite_failure_cancels_only_its_dependents() {
    let fixture = Fixture::new();
    write_native_spec(&fixture, "status-two", "args=[\"echo\"]");
    fixture.write("passing.ska", "fn main() -> i64 { return 0; }\n");
    fixture.write(
        "passing.golden.toml",
        r#"schema=1
[[test]]
name="passing"
mode="run"
source="passing.ska"
compiler_args=["--fake-mode","success"]
[[test.run]]
name="run"
args=["echo"]
"#,
    );
    let plan = fixture.plan();
    let selected = select(&plan, &SelectionOptions::default()).unwrap();

    let execution = execute_parallel(
        &selected,
        &fixture.options(Determinism::Off, "success"),
        scheduler(3, false),
    );

    assert!(!execution.passed());
    let failed = execution
        .leaves()
        .iter()
        .find(|leaf| leaf.leaf_id().contains("native"))
        .unwrap();
    assert!(matches!(failed.status(), StageStatus::Cancelled { .. }));
    let passing = execution
        .leaves()
        .iter()
        .find(|leaf| leaf.leaf_id().contains("passing"))
        .unwrap();
    assert!(passing.status().passed());
    assert_eq!(fs::read_to_string(&fixture.link_counter).unwrap(), "link\n");
}

#[test]
fn single_worker_and_parallel_execution_have_equal_ordered_semantics() {
    let fixture = Fixture::new();
    write_native_spec(
        &fixture,
        "success",
        "args=[\"echo\"]\nstdin={inline=\"first\"}\nexpect={stdout={inline=\"first\"},stderr={inline=\"first\"}}\n[[test.run]]\nname=\"second\"\nargs=[\"echo\"]\nstdin={inline=\"second\"}\nexpect={stdout={inline=\"second\"},stderr={inline=\"second\"}}",
    );
    write_compile_fail_spec(&fixture, "compile-fail", "error[FAKE001]");
    let plan = fixture.plan();
    let selected = select(&plan, &SelectionOptions::default()).unwrap();
    let options = fixture.options(Determinism::Off, "success");

    let single = execute_sequential(&selected, &options);
    let parallel = execute_parallel(&selected, &options, scheduler(4, false));

    assert_eq!(semantic_projection(&single), semantic_projection(&parallel));
}

fn semantic_projection(execution: &skald_golden::SequentialExecution) -> Vec<LeafSemantics> {
    execution
        .leaves()
        .iter()
        .map(|leaf| LeafSemantics {
            id: leaf.leaf_id().to_owned(),
            status: leaf.status().clone(),
            runs: leaf
                .repetitions()
                .iter()
                .map(|run| RunSemantics {
                    termination: run.observation().termination(),
                    stdout: run.observation().stdout().to_vec(),
                    stderr: run.observation().stderr().to_vec(),
                })
                .collect(),
        })
        .collect()
}

#[derive(Debug, PartialEq, Eq)]
struct LeafSemantics {
    id: String,
    status: StageStatus,
    runs: Vec<RunSemantics>,
}

#[derive(Debug, PartialEq, Eq)]
struct RunSemantics {
    termination: ProcessTermination,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}
