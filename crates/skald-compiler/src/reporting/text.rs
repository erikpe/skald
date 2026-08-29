//! Deterministic human rendering and writer-backed observation.

use std::{
    fmt::Write as _,
    io::{self, Write},
    time::Duration,
};

use super::{metrics::MetricValue, ReportDetail, ReportEvent, ReportObserver, ReportOutcome};

/// Renders one event for the selected human detail level.
///
/// The returned text is empty when reporting is off. Otherwise each record
/// begins with `skac:` and ends in exactly one newline. Metric order is the
/// order carried by the event.
pub fn render_event(event: &ReportEvent, detail: ReportDetail) -> String {
    if !detail.includes(ReportDetail::Phases) {
        return String::new();
    }

    let mut rendered = String::new();
    match event {
        ReportEvent::PhaseStarted { phase } => {
            let _ = writeln!(rendered, "skac: phase: {} started", phase.label());
        }
        ReportEvent::PhaseFinished {
            phase,
            elapsed,
            outcome,
            metrics,
        } => {
            render_completion(
                &mut rendered,
                "phase",
                phase.label(),
                *outcome,
                *elapsed,
                detail,
            );
            if detail.includes(ReportDetail::Details) {
                for metric in metrics {
                    render_metric(&mut rendered, metric.name(), metric.value());
                }
            }
        }
        ReportEvent::ModuleParsed {
            module,
            stage,
            tokens,
            outcome,
        } => {
            if detail.includes(ReportDetail::Trace) {
                let _ = writeln!(
                    rendered,
                    "skac: trace: {} parsed module {module}: {tokens} tokens, {}",
                    stage.label(),
                    outcome.label()
                );
            }
        }
        ReportEvent::ArtifactPublished { kind, path } => {
            let _ = writeln!(
                rendered,
                "skac: artifact: {} {}",
                kind.label(),
                path.display()
            );
        }
        ReportEvent::RunFinished {
            scope,
            elapsed,
            outcome,
        } => {
            render_completion(
                &mut rendered,
                "run",
                scope.label(),
                *outcome,
                *elapsed,
                detail,
            );
        }
    }
    rendered
}

fn render_completion(
    rendered: &mut String,
    category: &str,
    operation: &str,
    outcome: ReportOutcome,
    elapsed: Duration,
    detail: ReportDetail,
) {
    if detail.includes(ReportDetail::Details) {
        let _ = writeln!(
            rendered,
            "skac: {category}: {operation} {} in {}",
            outcome.label(),
            DurationDisplay(elapsed)
        );
    } else {
        let _ = writeln!(
            rendered,
            "skac: {category}: {operation} {}",
            outcome.label()
        );
    }
}

fn render_metric(rendered: &mut String, name: &str, value: MetricValue) {
    match value {
        MetricValue::Count(value) => {
            let _ = writeln!(rendered, "skac: stats: {name}: {value}");
        }
        MetricValue::Bytes(value) => {
            let unit = if value == 1 { "byte" } else { "bytes" };
            let _ = writeln!(rendered, "skac: stats: {name}: {value} {unit}");
        }
    }
}

struct DurationDisplay(Duration);

impl std::fmt::Display for DurationDisplay {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let rounded_micros = (self.0.as_nanos() + 500) / 1_000;
        write!(
            formatter,
            "{}.{:03} ms",
            rounded_micros / 1_000,
            rounded_micros % 1_000
        )
    }
}

/// A human-text observer that defers its first writer failure.
///
/// After a failure, the observer suppresses later writes and reports every
/// detail level as disabled. The caller may inspect or extract the retained
/// error after compilation without changing the observer trait's infallible
/// contract.
#[derive(Debug)]
pub struct TextObserver<W: Write> {
    writer: W,
    detail: ReportDetail,
    error: Option<io::Error>,
}

impl<W: Write> TextObserver<W> {
    pub fn new(writer: W, detail: ReportDetail) -> Self {
        Self {
            writer,
            detail,
            error: None,
        }
    }

    pub fn detail(&self) -> ReportDetail {
        self.detail
    }

    pub fn error(&self) -> Option<&io::Error> {
        self.error.as_ref()
    }

    pub fn into_parts(self) -> (W, Option<io::Error>) {
        (self.writer, self.error)
    }
}

impl<W: Write> ReportObserver for TextObserver<W> {
    fn enabled(&self, detail: ReportDetail) -> bool {
        self.error.is_none() && self.detail.includes(detail)
    }

    fn observe(&mut self, event: ReportEvent) {
        if self.error.is_some() {
            return;
        }

        let rendered = render_event(&event, self.detail);
        if rendered.is_empty() {
            return;
        }

        if let Err(error) = self.writer.write_all(rendered.as_bytes()) {
            self.error = Some(error);
        }
    }
}

#[cfg(test)]
mod duration_tests {
    use super::*;

    #[test]
    fn duration_display_rounds_to_nearest_microsecond() {
        assert_eq!(
            DurationDisplay(Duration::from_nanos(499)).to_string(),
            "0.000 ms"
        );
        assert_eq!(
            DurationDisplay(Duration::from_nanos(500)).to_string(),
            "0.001 ms"
        );
        assert_eq!(
            DurationDisplay(Duration::from_nanos(12_345_500)).to_string(),
            "12.346 ms"
        );
    }
}
