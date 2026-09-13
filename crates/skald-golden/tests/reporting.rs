mod support;

use skald_golden::{
    render_report, select, Determinism, Report, ReportFormat, ReportOptions, SelectionOptions,
};
use std::fs;
use support::{write_compile_fail_spec, write_native_spec, Fixture};

#[test]
fn report_counts_keep_specs_tests_builds_runs_and_processes_distinct() {
    let fixture = Fixture::new();
    write_native_spec(
        &fixture,
        "success",
        "args=[\"echo\"]\nstdin={inline=\"hello\\n\"}\nexpect={stdout={inline=\"hello\\n\"},stderr={inline=\"hello\\n\"}}",
    );
    write_compile_fail_spec(&fixture, "compile-fail", "error[FAKE001]");
    let plan = fixture.plan();
    let selected = select(&plan, &SelectionOptions::default()).unwrap();
    let execution = skald_golden::execute_parallel(
        &selected,
        &fixture.options(Determinism::Compile, "success"),
        skald_golden::SchedulerOptions::default(),
    );
    let report = Report::new(
        &selected,
        &execution,
        Determinism::Compile,
        ReportOptions::default().with_show_output(true),
    );

    assert!(report.passed());
    assert_eq!(report.counts.specs, 2);
    assert_eq!(report.counts.source_tests, 2);
    assert_eq!(report.counts.compile_fail_builds, 1);
    assert_eq!(report.counts.successful_builds, 1);
    assert_eq!(report.counts.named_runs, 1);
    assert_eq!(report.counts.compiler_processes, 4);
    assert_eq!(report.counts.links, 1);
    assert_eq!(report.counts.executions, 1);
    assert_eq!(report.counts.leaves_passed, 2);
    assert!(report.runtime.is_some());
    assert!(report.duration_ms >= 0.0);
    assert!(report.cases.windows(2).all(|pair| pair[0].id < pair[1].id));

    let native = report.cases.iter().find(|case| case.kind == "run").unwrap();
    assert_eq!(native.stages.len(), 3);
    assert_eq!(native.stages[0].stage, "compile");
    assert_eq!(native.stages[0].processes.len(), 2);
    assert_eq!(native.stages[1].stage, "link");
    assert_eq!(native.stages[2].stage, "execution");
    assert_eq!(native.stages[2].artifact_retained, Some(false));
    let native_stdout = native.stages[2].processes[0].stdout.as_ref().unwrap();
    assert_eq!(native_stdout.matchers.len(), 1);
    assert_eq!(native_stdout.matchers[0].status, "matched");
    assert!(native.stages.iter().all(|stage| stage.duration_ms >= 0.0));
    assert!(native
        .stages
        .iter()
        .all(|stage| !stage.processes.is_empty()));

    let compile_fail = report
        .cases
        .iter()
        .find(|case| case.kind == "compile-fail")
        .unwrap();
    let stderr = compile_fail.stages[0].processes[0].stderr.as_ref().unwrap();
    assert_eq!(stderr.policy.as_deref(), Some("contains"));
    assert_eq!(stderr.match_offset, Some(0));
    assert_eq!(stderr.matchers[0].status, "matched");

    let human = render_report(&report, ReportFormat::Human).unwrap();
    assert!(human.contains("matcher 0: matched, policy exact"));
}

#[test]
fn every_format_uses_the_same_canonical_ids_stages_and_statuses() {
    let fixture = Fixture::new();
    write_compile_fail_spec(&fixture, "compile-fail", "error[FAKE001]");
    let plan = fixture.plan();
    let selected = select(&plan, &SelectionOptions::default()).unwrap();
    let execution = fixture.execute(Determinism::Off);
    let report = Report::new(
        &selected,
        &execution,
        Determinism::Off,
        ReportOptions::default(),
    );
    let id = &report.cases[0].id;

    let human = render_report(&report, ReportFormat::Human).unwrap();
    assert!(human.contains(id));
    assert!(human.contains("PASS"));

    let json = render_report(&report, ReportFormat::Json).unwrap();
    let decoded: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded["cases"][0]["id"], id.as_str());
    assert_eq!(decoded["cases"][0]["status"], "passed");
    assert_eq!(decoded["cases"][0]["stages"][0]["stage"], "compile-fail");

    let junit = render_report(&report, ReportFormat::Junit).unwrap();
    assert!(junit.contains("failure::failure::default::&lt;compile&gt;"));
    assert!(junit.contains("compile-fail passed"));
}

#[test]
fn failure_counts_do_not_double_count_a_compile_fail_leaf_and_its_build() {
    let fixture = Fixture::new();
    write_compile_fail_spec(&fixture, "compile-fail", "not in the diagnostic");
    let plan = fixture.plan();
    let selected = select(&plan, &SelectionOptions::default()).unwrap();
    let execution = fixture.execute(Determinism::Off);
    let report = Report::new(
        &selected,
        &execution,
        Determinism::Off,
        ReportOptions::default(),
    );

    assert!(!report.passed());
    assert_eq!(report.counts.leaves_failed, 1);
    assert_eq!(report.counts.failures, 1);
    assert_eq!(report.counts.cancellations, 0);
}

#[test]
fn matcher_collections_are_structured_and_every_failure_is_rendered_in_order() {
    let fixture = Fixture::new();
    fixture.write("failure.ska", "fn main() -> i64 { return missing(); }\n");
    fixture.write("missing-one.stderr", "temporarily present");
    fixture.write("missing-two.stderr", "temporarily present");
    fixture.write(
        "failure.golden.toml",
        r#"
schema = 2
[[test]]
name = "failure"
mode = "compile-fail"
source = "failure.ska"
compiler_args = ["--fake-mode", "compile-fail-streams"]

[test.expect.stdout]
matches = [
  { name = "stdout header", match = "starts-with", inline = "compiler stdout" },
  { match = "contains", inline = "absent stdout" },
  { match = "exact", inline = "compiler stdout: alpha omega\n" },
]

[[test.expect.stderr.matches]]
name = "first diagnostic"
match = "contains"
inline = "error[FAKE001]: first rejected construct"

[[test.expect.stderr.matches]]
name = "missing <&> diagnostic"
match = "contains"
inline = "error[ABSENT]"

[[test.expect.stderr.matches]]
name = "unloadable one"
match = "contains"
file = "missing-one.stderr"

[[test.expect.stderr.matches]]
name = "unloadable two"
match = "starts-with"
file = "missing-two.stderr"
"#,
    );
    let plan = fixture.plan();
    fs::remove_file(fixture.root.join("missing-one.stderr")).unwrap();
    fs::remove_file(fixture.root.join("missing-two.stderr")).unwrap();
    let selected = select(&plan, &SelectionOptions::default()).unwrap();
    let execution =
        skald_golden::execute_sequential(&selected, &fixture.options(Determinism::Off, "success"));
    let report = Report::new(
        &selected,
        &execution,
        Determinism::Off,
        ReportOptions::default().with_show_output(true),
    );

    let stage = &report.cases[0].stages[0];
    let process = &stage.processes[0];
    let stdout = process.stdout.as_ref().unwrap();
    let stderr = process.stderr.as_ref().unwrap();
    assert_eq!(stdout.policy, None);
    assert_eq!(stdout.match_offset, None);
    assert_eq!(
        stdout
            .matchers
            .iter()
            .map(|matcher| matcher.status.as_str())
            .collect::<Vec<_>>(),
        ["matched", "mismatched", "matched"]
    );
    assert_eq!(stdout.matchers[0].match_offset, Some(0));
    assert_eq!(stdout.matchers[0].name.as_deref(), Some("stdout header"));
    assert_eq!(
        stderr
            .matchers
            .iter()
            .map(|matcher| matcher.status.as_str())
            .collect::<Vec<_>>(),
        ["matched", "mismatched", "load-failed", "load-failed"]
    );
    assert_eq!(stage.failures.len(), 4);
    assert!(stage.failures[0].message.contains("stdout matcher 1"));
    assert!(stage.failures[1]
        .message
        .contains("stderr matcher \"missing <&> diagnostic\""));
    assert!(stage.failures[2]
        .message
        .contains("stderr matcher \"unloadable one\""));
    assert!(stage.failures[3]
        .message
        .contains("stderr matcher \"unloadable two\""));

    let human = render_report(&report, ReportFormat::Human).unwrap();
    assert!(human.contains("matcher 0 \"stdout header\": matched"));
    assert!(human.contains("matcher 3 \"unloadable two\": load-failed"));

    let json = render_report(&report, ReportFormat::Json).unwrap();
    let decoded: serde_json::Value = serde_json::from_str(&json).unwrap();
    let json_stderr = &decoded["cases"][0]["stages"][0]["processes"][0]["stderr"];
    assert!(json_stderr["policy"].is_null());
    assert_eq!(json_stderr["matchers"][2]["status"], "load-failed");
    assert!(json_stderr["matchers"][2]["path"]
        .as_str()
        .unwrap()
        .contains("missing-one.stderr"));

    let junit = render_report(&report, ReportFormat::Junit).unwrap();
    assert_eq!(junit.matches("<failure ").count(), 4);
    let stdout_failure = junit.find("stdout matcher 1").unwrap();
    let named_failure = junit.find("missing &lt;&amp;&gt; diagnostic").unwrap();
    let first_load = junit.find("unloadable one").unwrap();
    let second_load = junit.find("unloadable two").unwrap();
    assert!(stdout_failure < named_failure);
    assert!(named_failure < first_load);
    assert!(first_load < second_load);
}

#[test]
fn passing_binary_stream_reports_share_one_capture_across_exact_and_partial_matchers() {
    let fixture = Fixture::new();
    fixture.write("program.ska", "fn main() -> i64 { return 0; }\n");
    fixture.write("expected.stdout", b"failure stdout\0\xff");
    fixture.write(
        "native.golden.toml",
        r#"
schema = 2
[[test]]
name = "native"
mode = "run"
source = "program.ska"
compiler_args = ["--fake-mode", "success"]
[[test.run]]
name = "binary"
args = ["fail"]
expect.exit = 17
expect.stdout.matches = [
  { match = "exact", file = "expected.stdout" },
  { name = "text fragment", match = "contains", inline = "stdout" },
]
expect.stderr = { inline = "failure stderr\n" }
"#,
    );
    let plan = fixture.plan();
    let selected = select(&plan, &SelectionOptions::default()).unwrap();
    let execution =
        skald_golden::execute_sequential(&selected, &fixture.options(Determinism::Off, "success"));
    let report = Report::new(
        &selected,
        &execution,
        Determinism::Off,
        ReportOptions::default().with_show_output(true),
    );

    assert!(report.passed());
    let stdout = report.cases[0].stages[2].processes[0]
        .stdout
        .as_ref()
        .unwrap();
    assert_eq!(stdout.escaped, "failure stdout\\x00\\xff");
    assert_eq!(stdout.policy, None);
    assert_eq!(stdout.matchers.len(), 2);
    assert_eq!(stdout.matchers[0].policy, "exact");
    assert_eq!(stdout.matchers[1].policy, "contains");
    assert_eq!(stdout.matchers[1].match_offset, Some(8));

    let human = render_report(&report, ReportFormat::Human).unwrap();
    assert!(human.contains("stdout: 16 bytes, b\"failure stdout\\x00\\xff\""));
    assert!(human.contains("matcher 1 \"text fragment\": matched, policy contains"));
}
