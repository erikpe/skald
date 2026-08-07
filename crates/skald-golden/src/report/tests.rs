use super::{bytes, model::*, render};

fn sample_report() -> Report {
    Report {
        schema_version: 1,
        determinism: "compile".to_owned(),
        duration_ms: 12.5,
        counts: ReportCounts {
            specs: 1,
            source_tests: 1,
            compile_fail_builds: 1,
            leaves_failed: 1,
            compiler_processes: 2,
            failures: 1,
            ..ReportCounts::default()
        },
        runtime: None,
        cases: vec![CaseReport {
            id: "language/types::bad<&>::default::<compile>".to_owned(),
            spec_id: "language/types".to_owned(),
            test_id: "language/types::bad<&>".to_owned(),
            build_id: "language/types::bad<&>::default".to_owned(),
            kind: "compile-fail".to_owned(),
            status: "failed".to_owned(),
            duration_ms: 12.0,
            stages: vec![StageReport {
                stage: "compile-fail".to_owned(),
                status: "failed".to_owned(),
                duration_ms: 12.0,
                artifact_directory: Some("b\"/artifacts\"".to_owned()),
                artifact_retained: Some(true),
                processes: vec![ProcessReport {
                    repetition: 1,
                    command: "b\"skac\" b\"bad source.ska\"".to_owned(),
                    working_directory: "b\"/work\"".to_owned(),
                    duration_ms: 12.0,
                    termination: Some("exit code 1".to_owned()),
                    stdout: None,
                    stderr: Some(StreamReport {
                        length: 7,
                        escaped: "actual\\n".to_owned(),
                        policy: Some("starts-with".to_owned()),
                        match_offset: None,
                        matchers: vec![MatcherReport {
                            index: 0,
                            name: Some("diagnostic <&>".to_owned()),
                            policy: "starts-with".to_owned(),
                            status: "mismatched".to_owned(),
                            match_offset: None,
                            expected_length: Some(9),
                            expected: Some("expected\\n".to_owned()),
                            path: None,
                            error: None,
                        }],
                    }),
                }],
                failures: vec![
                    FailureReport {
                        kind: "stderr".to_owned(),
                        message: "stderr did not satisfy starts-with matching".to_owned(),
                        policy: Some("starts-with".to_owned()),
                        expected_length: Some(9),
                        actual_length: Some(7),
                        expected: Some("expected\\n".to_owned()),
                        actual: Some("actual\\n".to_owned()),
                        diff: Some("--- expected\n+++ actual\n-expected\n+actual\n".to_owned()),
                    },
                    FailureReport {
                        kind: "stderr".to_owned(),
                        message: "stderr matcher \"second <&>\" did not satisfy contains matching"
                            .to_owned(),
                        policy: Some("contains".to_owned()),
                        expected_length: Some(6),
                        actual_length: Some(7),
                        expected: Some("second".to_owned()),
                        actual: Some("actual\\n".to_owned()),
                        diff: Some("--- expected\n+++ actual\n-second\n+actual\n".to_owned()),
                    },
                ],
            }],
        }],
        scheduler_failure: None,
        options: ReportOptions::default().with_slowest(std::num::NonZeroUsize::new(1)),
    }
}

#[test]
fn escapes_binary_data_and_bounds_large_values() {
    assert_eq!(bytes::escape_bytes(b"a\0\n\xff"), "a\\x00\\n\\xff");
    let escaped = bytes::escape_bytes(&vec![b'x'; 600]);
    assert!(escaped.contains("88 bytes omitted"));
}

#[test]
fn emits_bounded_utf8_diffs_only_for_text() {
    let rendered = bytes::diff(b"expected\n", b"actual\n").unwrap();
    assert!(rendered.contains("--- expected\n+++ actual"));
    assert!(rendered.contains("-expected\n+actual"));
    assert!(bytes::diff(b"\xff", b"actual").is_none());

    let large = vec![b'x'; 20_000];
    assert!(bytes::diff(&large, b"different")
        .unwrap()
        .contains("diff truncated"));
}

#[test]
fn human_reports_all_failure_context_and_stable_slowest_ids() {
    let output = render(&sample_report(), ReportFormat::Human).unwrap();
    for expected in [
        "FAIL language/types::bad<&>::default::<compile>",
        "stage compile-fail: failed",
        "command: b\"skac\" b\"bad source.ska\"",
        "working directory: b\"/work\"",
        "termination: exit code 1",
        "policy: starts-with",
        "bytes: expected 9, actual 7",
        "--- expected",
        "slowest 1:",
        "2 compiler processes, 0 links, 0 executions",
    ] {
        assert!(
            output.contains(expected),
            "missing {expected:?} in:\n{output}"
        );
    }
    assert_eq!(output.matches("actual:   b\"actual\\n\"").count(), 1);
    assert!(output.contains("matcher 0 \"diagnostic <&>\": mismatched"));
}

#[test]
fn json_and_junit_encode_the_same_canonical_case_and_stage() {
    let report = sample_report();
    let json = render(&report, ReportFormat::Json).unwrap();
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(value["cases"][0]["id"], report.cases[0].id);
    assert_eq!(value["cases"][0]["stages"][0]["stage"], "compile-fail");
    assert_eq!(value["cases"][0]["duration_ms"], 12.0);
    let matcher = &value["cases"][0]["stages"][0]["processes"][0]["stderr"]["matchers"][0];
    assert_eq!(matcher["name"], "diagnostic <&>");
    assert_eq!(matcher["status"], "mismatched");

    let junit = render(&report, ReportFormat::Junit).unwrap();
    let mut reader = quick_xml::Reader::from_str(&junit);
    let mut depth = 0usize;
    loop {
        match reader.read_event().unwrap() {
            quick_xml::events::Event::Start(_) => depth += 1,
            quick_xml::events::Event::End(_) => depth -= 1,
            quick_xml::events::Event::Eof => break,
            _ => {}
        }
    }
    assert_eq!(depth, 0);
    assert!(junit.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>"));
    assert!(junit.contains("tests=\"1\" failures=\"1\" skipped=\"0\""));
    assert!(junit.contains("language/types::bad&lt;&amp;&gt;::default::&lt;compile&gt;"));
    assert!(junit.contains("compile-fail: stderr did not satisfy starts-with matching"));
    assert!(junit.contains("stderr matcher &quot;second &lt;&amp;&gt;&quot;"));
    assert_eq!(junit.matches("<failure ").count(), 2);
    assert!(junit.ends_with("</testsuite>\n"));
}
