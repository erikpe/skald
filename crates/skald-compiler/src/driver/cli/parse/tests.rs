use super::*;

fn options(arguments: &[&str]) -> CompileOptions {
    let command = parse_command(arguments.iter().map(OsString::from)).unwrap();
    let Command::Compile(options) = command else {
        panic!("expected compile command");
    };
    options
}

fn error(arguments: &[&str]) -> String {
    parse_command(arguments.iter().map(OsString::from))
        .err()
        .expect("expected command error")
}

#[test]
fn report_shorthand_uses_saturating_subtraction_and_caps_at_trace() {
    let cases = [
        (&["skac", "main.ska"][..], ReportDetail::Off),
        (&["skac", "main.ska", "-v"][..], ReportDetail::Phases),
        (&["skac", "-vv", "main.ska"][..], ReportDetail::Details),
        (&["skac", "-vvv", "main.ska"][..], ReportDetail::Trace),
        (&["skac", "-vvvvvvvv", "main.ska"][..], ReportDetail::Trace),
        (&["skac", "-q", "main.ska"][..], ReportDetail::Off),
        (&["skac", "-vv", "-q", "main.ska"][..], ReportDetail::Phases),
        (&["skac", "-vvqqq", "main.ska"][..], ReportDetail::Off),
        (
            &["skac", "-vvvv", "-qq", "main.ska"][..],
            ReportDetail::Details,
        ),
    ];

    for (arguments, expected) in cases {
        assert_eq!(options(arguments).report_detail, expected, "{arguments:?}");
    }
    assert_eq!(
        resolve_report_detail(usize::MAX, 0, None).unwrap(),
        ReportDetail::Trace
    );
    assert_eq!(
        resolve_report_detail(usize::MAX, usize::MAX, None).unwrap(),
        ReportDetail::Off
    );
    assert_eq!(
        resolve_report_detail(0, usize::MAX, None).unwrap(),
        ReportDetail::Off
    );
}

#[test]
fn explicit_report_and_diagnostic_levels_are_typed() {
    let cases = [
        ("off", ReportDetail::Off),
        ("phases", ReportDetail::Phases),
        ("details", ReportDetail::Details),
        ("trace", ReportDetail::Trace),
    ];
    for (value, expected) in cases {
        assert_eq!(
            options(&["skac", "main.ska", "--report-level", value]).report_detail,
            expected
        );
    }

    assert_eq!(
        options(&["skac", "main.ska"]).diagnostic_level,
        DiagnosticLevel::Warning
    );
    assert_eq!(
        options(&["skac", "main.ska", "--diagnostic-level", "error"]).diagnostic_level,
        DiagnosticLevel::Error
    );
}

#[test]
fn explicit_levels_reject_conflicts_repetition_and_invalid_values() {
    let cases = [
        (
            &["skac", "main.ska", "-v", "--report-level", "trace"][..],
            "cannot be combined",
        ),
        (
            &["skac", "--report-level", "off", "-q", "main.ska"][..],
            "cannot be combined",
        ),
        (
            &[
                "skac",
                "main.ska",
                "--report-level",
                "off",
                "--report-level",
                "trace",
            ][..],
            "report level specified more than once",
        ),
        (
            &[
                "skac",
                "main.ska",
                "--diagnostic-level",
                "warning",
                "--diagnostic-level",
                "error",
            ][..],
            "diagnostic level specified more than once",
        ),
        (
            &["skac", "main.ska", "--report-level", "verbose"][..],
            "invalid report level `verbose`",
        ),
        (
            &["skac", "main.ska", "--diagnostic-level", "off"][..],
            "invalid diagnostic level `off`",
        ),
        (&["skac", "main.ska", "-vx"][..], "unknown option `-vx`"),
    ];

    for (arguments, expected) in cases {
        assert!(error(arguments).contains(expected), "{arguments:?}");
    }
}
