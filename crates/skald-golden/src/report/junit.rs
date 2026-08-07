use super::model::{CaseReport, Report};
use std::fmt::Write as _;

pub(super) fn render(report: &Report) -> String {
    let failures = report
        .cases
        .iter()
        .filter(|case| case.status == "failed")
        .count()
        + usize::from(report.scheduler_failure.is_some());
    let skipped = report
        .cases
        .iter()
        .filter(|case| case.status == "cancelled")
        .count();
    let runtime_failure = usize::from(
        report
            .runtime
            .as_ref()
            .is_some_and(|runtime| runtime.status == "failed"),
    );
    let failures = failures + runtime_failure;
    let tests = report.cases.len()
        + usize::from(report.scheduler_failure.is_some())
        + usize::from(report.runtime.is_some());
    let mut output = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    writeln!(
        output,
        "<testsuite name=\"skald-golden\" tests=\"{tests}\" failures=\"{failures}\" skipped=\"{skipped}\" time=\"{:.6}\">",
        report.duration_ms / 1_000.0
    )
    .expect("writing to a string cannot fail");
    if let Some(runtime) = &report.runtime {
        writeln!(
            output,
            "  <testcase name=\"runtime\" classname=\"skald-golden.runtime\" time=\"{:.6}\">",
            runtime.duration_ms / 1_000.0
        )
        .expect("writing to a string cannot fail");
        if runtime.status == "failed" {
            render_failures(&mut output, "runtime", &runtime.failures);
        }
        writeln!(
            output,
            "    <system-out>{} {} {:.3}ms</system-out>",
            runtime.stage, runtime.status, runtime.duration_ms
        )
        .expect("writing to a string cannot fail");
        output.push_str("  </testcase>\n");
    }
    writeln!(
        output,
        "  <properties><property name=\"determinism\" value=\"{}\"/></properties>",
        xml(&report.determinism)
    )
    .expect("writing to a string cannot fail");
    for case in &report.cases {
        render_case(&mut output, case);
    }
    if let Some(failure) = &report.scheduler_failure {
        writeln!(
            output,
            "  <testcase name=\"scheduler\" classname=\"skald-golden.internal\">"
        )
        .expect("writing to a string cannot fail");
        writeln!(
            output,
            "    <failure message=\"{}\">{}</failure>",
            xml(&failure.message),
            xml(&format!(
                "active: {}\npending: {}",
                failure.active_nodes.join(", "),
                failure.pending_nodes.join(", ")
            ))
        )
        .expect("writing to a string cannot fail");
        output.push_str("  </testcase>\n");
    }
    output.push_str("</testsuite>\n");
    output
}

fn render_case(output: &mut String, case: &CaseReport) {
    writeln!(
        output,
        "  <testcase name=\"{}\" classname=\"skald-golden.{}\" time=\"{:.6}\">",
        xml(&case.id),
        xml(&case.kind),
        case.duration_ms / 1_000.0
    )
    .expect("writing to a string cannot fail");
    if case.status == "cancelled" {
        output.push_str("    <skipped message=\"cancelled by dependency\"/>\n");
    } else if case.status == "failed" {
        for stage in &case.stages {
            render_failures(output, &stage.stage, &stage.failures);
        }
    }
    let stages = case
        .stages
        .iter()
        .map(|stage| {
            let mut line = format!(
                "{} {} {:.3}ms",
                stage.stage, stage.status, stage.duration_ms
            );
            for process in &stage.processes {
                write!(
                    line,
                    "\n  process {} {:.3}ms{}: {}",
                    process.repetition,
                    process.duration_ms,
                    process
                        .termination
                        .as_ref()
                        .map(|value| format!(" ({value})"))
                        .unwrap_or_default(),
                    process.command,
                )
                .expect("writing to a string cannot fail");
            }
            line
        })
        .collect::<Vec<_>>()
        .join("\n");
    writeln!(output, "    <system-out>{}</system-out>", xml(&stages))
        .expect("writing to a string cannot fail");
    output.push_str("  </testcase>\n");
}

fn render_failures(output: &mut String, stage: &str, failures: &[super::model::FailureReport]) {
    for failure in failures {
        writeln!(
            output,
            "    <failure message=\"{}\">{}</failure>",
            xml(&format!("{stage}: {}", failure.message)),
            xml(&failure_text(failure))
        )
        .expect("writing to a string cannot fail");
    }
}

fn failure_text(failure: &super::model::FailureReport) -> String {
    let mut output = failure.message.clone();
    if let Some(policy) = &failure.policy {
        write!(output, "\npolicy: {policy}").expect("writing to a string cannot fail");
    }
    if let (Some(expected), Some(actual)) = (failure.expected_length, failure.actual_length) {
        write!(output, "\nbytes: expected {expected}, actual {actual}")
            .expect("writing to a string cannot fail");
    }
    if let Some(expected) = &failure.expected {
        write!(output, "\nexpected: b\"{expected}\"").expect("writing to a string cannot fail");
    }
    if let Some(actual) = &failure.actual {
        write!(output, "\nactual: b\"{actual}\"").expect("writing to a string cannot fail");
    }
    if let Some(diff) = &failure.diff {
        output.push('\n');
        output.push_str(diff);
    }
    output
}

fn xml(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            '\"' => output.push_str("&quot;"),
            '\'' => output.push_str("&apos;"),
            character if character.is_control() && !matches!(character, '\n' | '\r' | '\t') => {
                output.push('\u{fffd}');
            }
            character => output.push(character),
        }
    }
    output
}
