use super::model::{CaseReport, FailureReport, Report, StageReport};
use std::fmt::Write as _;

pub(super) fn render(report: &Report) -> String {
    let mut output = String::new();
    if let Some(runtime) = &report.runtime {
        if runtime.status != "passed" || report.options.show_output() {
            output.push_str("RUNTIME\n");
            render_stage(&mut output, runtime, report.options.show_output());
        }
    }
    for case in &report.cases {
        let label = match case.status.as_str() {
            "passed" => "PASS",
            "failed" => "FAIL",
            "cancelled" => "CANCELLED",
            _ => "UNKNOWN",
        };
        writeln!(output, "{} {} ({:.3} ms)", label, case.id, case.duration_ms)
            .expect("writing to a string cannot fail");
        if case.status != "passed" || report.options.show_output() {
            render_case(&mut output, case, report.options.show_output());
        }
    }
    if let Some(failure) = &report.scheduler_failure {
        writeln!(output, "FAIL scheduler: {}", failure.message)
            .expect("writing to a string cannot fail");
        writeln!(output, "  active: {}", failure.active_nodes.join(", "))
            .expect("writing to a string cannot fail");
        writeln!(output, "  pending: {}", failure.pending_nodes.join(", "))
            .expect("writing to a string cannot fail");
    }
    if let Some(limit) = report.options.slowest() {
        let mut cases = report.cases.iter().collect::<Vec<_>>();
        cases.sort_by(|left, right| {
            right
                .duration_ms
                .total_cmp(&left.duration_ms)
                .then_with(|| left.id.cmp(&right.id))
        });
        writeln!(output, "slowest {}:", limit.get().min(cases.len()))
            .expect("writing to a string cannot fail");
        for case in cases.into_iter().take(limit.get()) {
            writeln!(output, "  {:.3} ms  {}", case.duration_ms, case.id)
                .expect("writing to a string cannot fail");
        }
    }
    writeln!(
        output,
        "golden: {} passed, {} failed, {} cancelled; {} stage failures, {} stage cancellations; {} compiler processes, {} links, {} executions; {:.3}s",
        report.counts.leaves_passed,
        report.counts.leaves_failed,
        report.counts.leaves_cancelled,
        report.counts.failures,
        report.counts.cancellations,
        report.counts.compiler_processes,
        report.counts.links,
        report.counts.executions,
        report.duration_ms / 1_000.0,
    )
    .expect("writing to a string cannot fail");
    output
}

fn render_case(output: &mut String, case: &CaseReport, show_output: bool) {
    for stage in &case.stages {
        if stage.status != "passed" || show_output {
            render_stage(output, stage, show_output);
        }
    }
}

fn render_stage(output: &mut String, stage: &StageReport, show_output: bool) {
    writeln!(
        output,
        "  stage {}: {} ({:.3} ms)",
        stage.stage, stage.status, stage.duration_ms
    )
    .expect("writing to a string cannot fail");
    if let Some(value) = &stage.artifact_directory {
        writeln!(output, "    artifact directory: {value}")
            .expect("writing to a string cannot fail");
    }
    if let (Some(path), Some(retained)) = (&stage.artifact_directory, stage.artifact_retained) {
        writeln!(
            output,
            "    artifact retention: {} at {path}",
            if retained { "retained" } else { "removed" }
        )
        .expect("writing to a string cannot fail");
    }
    for process in &stage.processes {
        let indent = if stage.processes.len() > 1 {
            writeln!(
                output,
                "    process {} ({:.3} ms):",
                process.repetition, process.duration_ms
            )
            .expect("writing to a string cannot fail");
            "      "
        } else {
            "    "
        };
        writeln!(output, "{indent}command: {}", process.command)
            .expect("writing to a string cannot fail");
        writeln!(
            output,
            "{indent}working directory: {}",
            process.working_directory
        )
        .expect("writing to a string cannot fail");
        if let Some(termination) = &process.termination {
            writeln!(output, "{indent}termination: {termination}")
                .expect("writing to a string cannot fail");
        }
        if show_output || stage.status != "passed" {
            for (name, stream) in [("stdout", &process.stdout), ("stderr", &process.stderr)] {
                if let Some(stream) = stream {
                    write!(output, "{indent}{name}: {} bytes", stream.length)
                        .expect("writing to a string cannot fail");
                    if let Some(policy) = &stream.policy {
                        write!(output, ", policy {policy}")
                            .expect("writing to a string cannot fail");
                    }
                    if let Some(offset) = stream.match_offset {
                        write!(output, ", match offset {offset}")
                            .expect("writing to a string cannot fail");
                    }
                    writeln!(output, ", b\"{}\"", stream.escaped)
                        .expect("writing to a string cannot fail");
                }
            }
        }
    }
    for failure in &stage.failures {
        render_failure(output, failure);
    }
}

fn render_failure(output: &mut String, failure: &FailureReport) {
    writeln!(output, "    mismatch {}: {}", failure.kind, failure.message)
        .expect("writing to a string cannot fail");
    if let Some(policy) = &failure.policy {
        writeln!(output, "      policy: {policy}").expect("writing to a string cannot fail");
    }
    if let (Some(expected), Some(actual)) = (failure.expected_length, failure.actual_length) {
        writeln!(output, "      bytes: expected {expected}, actual {actual}")
            .expect("writing to a string cannot fail");
    }
    if let Some(expected) = &failure.expected {
        writeln!(output, "      expected: b\"{expected}\"")
            .expect("writing to a string cannot fail");
    }
    if let Some(actual) = &failure.actual {
        writeln!(output, "      actual:   b\"{actual}\"").expect("writing to a string cannot fail");
    }
    if let Some(diff) = &failure.diff {
        for line in diff.lines() {
            writeln!(output, "      {line}").expect("writing to a string cannot fail");
        }
    }
}
