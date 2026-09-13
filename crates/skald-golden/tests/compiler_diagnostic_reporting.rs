mod support;

use skald_golden::{
    render_report, select, CompilationIssue, Determinism, Report, ReportFormat, ReportOptions,
    SelectionOptions, SequentialOptions,
};
use support::Fixture;

#[test]
fn compiler_reports_use_normalized_stderr_without_replacing_raw_capture() {
    let fixture = Fixture::new();
    fixture.write(
        "diagnostics/case/application/app.ska",
        "fn main() -> i64 { return missing(); }\n",
    );
    fixture.write(
        "diagnostics/case/dependencies/value.ska",
        "public fn value() -> i64 { return 0; }\n",
    );
    fixture.write(
        "diagnostics/paths.golden.toml",
        r#"
schema = 1
[[test]]
name = "paths"
mode = "compile-fail"
compiler_args = [
  "--entry", "app",
  "--module-root", "case/application",
  "--module-root", "case/dependencies",
  "--no-stdlib",
  "--fake-mode", "compile-fail-path",
]
expect.stderr = { match = "exact", inline = "error[FAKE001]: rejected module\n --> application/app.ska:1:1\n" }
"#,
    );

    let plan = fixture.plan();
    let selected = select(&plan, &SelectionOptions::default()).unwrap();
    let execution =
        skald_golden::execute_sequential(&selected, &fixture.options(Determinism::Off, "success"));
    assert!(execution.passed());

    let compilation = execution.builds()[0].compilation();
    let observation = &compilation.observations()[0];
    let raw = observation.process().unwrap().stderr();
    let normalized = b"error[FAKE001]: rejected module\n --> application/app.ska:1:1\n";
    let prefix = format!(
        "{}{}",
        fixture.root.join("diagnostics/case").display(),
        std::path::MAIN_SEPARATOR
    );
    assert!(raw
        .windows(prefix.len())
        .any(|window| window == prefix.as_bytes()));
    assert_ne!(raw, normalized);
    assert_eq!(observation.stderr_for_comparison(), Some(&normalized[..]));
    assert_eq!(
        compilation.stderr_comparison().unwrap().actual(),
        normalized
    );

    let report = Report::new(
        &selected,
        &execution,
        Determinism::Off,
        ReportOptions::default().with_show_output(true),
    );
    let stderr = report.cases[0].stages[0].processes[0]
        .stderr
        .as_ref()
        .unwrap();
    assert_eq!(stderr.length, normalized.len());
    assert!(stderr.escaped.contains("application/app.ska"));
    assert!(!stderr.escaped.contains(&fixture.root.to_string_lossy()[..]));
}

#[test]
fn repeated_binary_diagnostics_keep_raw_processes_and_portable_reports() {
    let fixture = Fixture::new();
    write_diagnostic_modules(&fixture);
    let normalized = normalized_path_diagnostic();
    fixture.write("diagnostics/expected.stderr", &normalized);
    fixture.write(
        "diagnostics/paths.golden.toml",
        r#"
schema = 2
[[test]]
name = "paths"
mode = "compile-fail"
compiler_args = [
  "--entry", "app",
  "--module-root", "case/application",
  "--module-root", "case/dependencies",
  "--no-stdlib",
  "--fake-mode", "compile-fail-paths",
]
[test.expect.stderr]
matches = [
  { name = "complete diagnostic", match = "exact", file = "expected.stderr" },
  { name = "primary path", match = "contains", inline = "application/app.ska" },
  { name = "message path", match = "contains", inline = "rejected path dependencies/value.ska" },
  { name = "missing detail", match = "contains", inline = "not emitted" },
]
"#,
    );

    let plan = fixture.plan();
    let selected = select(&plan, &SelectionOptions::default()).unwrap();
    let execution = skald_golden::execute_sequential(
        &selected,
        &fixture.options(Determinism::Compile, "success"),
    );
    let compilation = execution.builds()[0].compilation();
    assert_eq!(compilation.observations().len(), 2);
    assert!(!compilation
        .issues()
        .contains(&CompilationIssue::NondeterministicDiagnostics));

    let prefix = diagnostic_path_prefix(&fixture);
    let raw = compilation
        .observations()
        .iter()
        .map(|observation| observation.process().unwrap().stderr())
        .collect::<Vec<_>>();
    assert_ne!(raw[0], raw[1]);
    assert_eq!(byte_occurrences(raw[0], prefix.as_bytes()), 2);
    assert_eq!(byte_occurrences(raw[1], prefix.as_bytes()), 3);
    assert!(raw.iter().all(|stderr| stderr.contains(&0xff)));
    assert!(raw.iter().all(|stderr| stderr.contains(&0)));
    assert!(compilation
        .observations()
        .iter()
        .all(|observation| observation.stderr_for_comparison() == Some(&normalized)));

    let comparison = compilation.stderr_comparison().unwrap();
    assert_eq!(comparison.actual(), normalized);
    let report = Report::new(
        &selected,
        &execution,
        Determinism::Compile,
        ReportOptions::default().with_show_output(true),
    );
    let stderr = report.cases[0].stages[0].processes[0]
        .stderr
        .as_ref()
        .unwrap();
    assert_eq!(stderr.escaped, escaped_path_diagnostic());
    assert_eq!(
        stderr
            .matchers
            .iter()
            .map(|matcher| (
                matcher.name.as_deref(),
                matcher.policy.as_str(),
                matcher.status.as_str()
            ))
            .collect::<Vec<_>>(),
        [
            (Some("complete diagnostic"), "exact", "matched"),
            (Some("primary path"), "contains", "matched"),
            (Some("message path"), "contains", "matched"),
            (Some("missing detail"), "contains", "mismatched"),
        ]
    );
    assert_eq!(stderr.matchers[0].match_offset, Some(0));
    assert_eq!(
        stderr.matchers[1].match_offset,
        find_bytes(&normalized, b"application/app.ska")
    );
    assert_eq!(
        stderr.matchers[2].match_offset,
        find_bytes(&normalized, b"rejected path dependencies/value.ska")
    );

    let raw_primary_path = format!(" --> {prefix}application/app.ska");
    let human = render_report(&report, ReportFormat::Human).unwrap();
    assert!(human.contains("matcher 0 \"complete diagnostic\": matched, policy exact"));
    assert!(human.contains("\\xfferror[FAKE001]"));
    assert!(!human.contains(&raw_primary_path));

    let json = render_report(&report, ReportFormat::Json).unwrap();
    let decoded: serde_json::Value = serde_json::from_str(&json).unwrap();
    let json_stderr = &decoded["cases"][0]["stages"][0]["processes"][0]["stderr"];
    assert_eq!(json_stderr["escaped"], escaped_path_diagnostic());
    assert_eq!(
        json_stderr
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["escaped", "length", "match_offset", "matchers", "policy"]
    );
    assert_eq!(json_stderr["matchers"][2]["name"], "message path");
    assert!(!json.contains(&raw_primary_path));

    let junit = render_report(&report, ReportFormat::Junit).unwrap();
    assert!(junit.contains("\\xfferror[FAKE001]"));
    assert!(junit.contains("application/app.ska"));
    assert!(!junit.contains(&raw_primary_path));
}

#[test]
fn diagnostic_determinism_detects_differences_outside_normalized_paths() {
    let fixture = Fixture::new();
    write_diagnostic_modules(&fixture);
    fixture.write(
        "diagnostics/paths.golden.toml",
        r#"
schema = 1
[[test]]
name = "paths"
mode = "compile-fail"
compiler_args = [
  "--entry", "app",
  "--module-root", "case/application",
  "--module-root", "case/dependencies",
  "--no-stdlib",
  "--fake-mode", "compile-fail-paths",
  "--fake-vary-diagnostic",
]
expect.stderr = { match = "contains", inline = "error[FAKE001]" }
"#,
    );

    let plan = fixture.plan();
    let selected = select(&plan, &SelectionOptions::default()).unwrap();
    let execution = skald_golden::execute_sequential(
        &selected,
        &fixture.options(Determinism::Compile, "success"),
    );
    let compilation = execution.builds()[0].compilation();
    assert!(compilation
        .issues()
        .contains(&CompilationIssue::NondeterministicDiagnostics));
    let observations = compilation.observations();
    assert_ne!(
        observations[0].stderr_for_comparison(),
        observations[1].stderr_for_comparison()
    );

    let report = Report::new(
        &selected,
        &execution,
        Determinism::Compile,
        ReportOptions::default(),
    );
    let raw_primary_path = format!(
        " --> {}application/app.ska",
        diagnostic_path_prefix(&fixture)
    );
    for format in [ReportFormat::Human, ReportFormat::Json, ReportFormat::Junit] {
        let rendered = render_report(&report, format).unwrap();
        assert!(rendered.contains("different diagnostics"));
        assert!(!rendered.contains(&raw_primary_path));
    }
}

#[test]
fn compiler_capture_overflow_remains_visible_after_path_normalization() {
    let fixture = Fixture::new();
    write_diagnostic_modules(&fixture);
    fixture.write(
        "diagnostics/paths.golden.toml",
        r#"
schema = 1
[[test]]
name = "paths"
mode = "compile-fail"
compiler_args = [
  "--entry", "app",
  "--module-root", "case/application",
  "--module-root", "case/dependencies",
  "--no-stdlib",
  "--fake-mode", "compile-fail-paths",
]
expect.stderr = { match = "contains", inline = "error[FAKE001]" }
"#,
    );
    let plan = fixture.plan();
    let selected = select(&plan, &SelectionOptions::default()).unwrap();
    let base = fixture.options(Determinism::Off, "success");
    let capture_limit = normalized_path_diagnostic().len() + diagnostic_path_prefix(&fixture).len();
    let options = SequentialOptions::new(
        base.compiler().clone().with_capture_limit(capture_limit),
        base.runtime().clone(),
        base.toolchain().clone(),
        base.execution().clone(),
    )
    .with_linker_environment(base.linker_environment().clone())
    .with_linker_timeout(base.linker_timeout());
    let execution = skald_golden::execute_sequential(&selected, &options);
    let compilation = execution.builds()[0].compilation();
    let process = compilation.observations()[0].process().unwrap();
    assert_eq!(process.stderr().len(), capture_limit);
    assert_eq!(process.capture_overflows().len(), 1);
    assert!(process.capture_overflows()[0].observed() > capture_limit);
    assert!(compilation
        .issues()
        .iter()
        .any(|issue| matches!(issue, CompilationIssue::CaptureOverflow(overflow) if overflow == &process.capture_overflows()[0])));

    let report = Report::new(
        &selected,
        &execution,
        Determinism::Off,
        ReportOptions::default(),
    );
    assert!(report.cases[0].stages[0]
        .failures
        .iter()
        .any(|failure| failure.kind == "capture-limit"));
}

fn write_diagnostic_modules(fixture: &Fixture) {
    fixture.write(
        "diagnostics/case/application/app.ska",
        "fn main() -> i64 { return missing(); }\n",
    );
    fixture.write(
        "diagnostics/case/dependencies/value.ska",
        "public fn value() -> i64 { return 0; }\n",
    );
}

fn diagnostic_path_prefix(fixture: &Fixture) -> String {
    format!(
        "{}{}",
        fixture.root.join("diagnostics/case").display(),
        std::path::MAIN_SEPARATOR
    )
}

fn normalized_path_diagnostic() -> Vec<u8> {
    b"\xfferror[FAKE001]: rejected module\n --> application/app.ska:1:1\nnote: rejected path dependencies/value.ska\0\n".to_vec()
}

fn escaped_path_diagnostic() -> &'static str {
    "\\xfferror[FAKE001]: rejected module\\n --> application/app.ska:1:1\\nnote: rejected path dependencies/value.ska\\x00\\n"
}

fn byte_occurrences(haystack: &[u8], needle: &[u8]) -> usize {
    haystack
        .windows(needle.len())
        .filter(|window| *window == needle)
        .count()
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}
