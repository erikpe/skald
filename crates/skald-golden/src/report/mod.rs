//! Canonical report modeling and deterministic human and machine rendering.

mod bytes;
mod human;
mod junit;
mod model;

pub use model::{
    CaseReport, FailureReport, MatcherReport, ProcessReport, Report, ReportCounts, ReportFormat,
    ReportOptions, SchedulerFailureReport, StageReport, StreamReport,
};

/// Renders one canonical report without changing execution semantics.
pub fn render(report: &Report, format: ReportFormat) -> Result<String, String> {
    match format {
        ReportFormat::Human => Ok(human::render(report)),
        ReportFormat::Json => serde_json::to_string_pretty(report)
            .map(|mut value| {
                value.push('\n');
                value
            })
            .map_err(|error| format!("could not encode JSON report: {error}")),
        ReportFormat::Junit => Ok(junit::render(report)),
    }
}

pub(crate) use bytes::{diff, escape_bytes, escape_command, escape_path};

#[cfg(test)]
mod tests;
