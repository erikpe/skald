mod support;

use skald_golden::{
    render_report, select, Determinism, Report, ReportFormat, ReportOptions, SelectionOptions,
};
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
