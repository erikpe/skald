//! Request-scoped structured compiler observations.
//!
//! Reporting is operational data rather than source diagnostics or phase
//! dumps. The typed model and observers are available to repository tools,
//! but compiler phases and the command-line driver do not emit events yet.

mod event;
mod metrics;
mod text;

pub use event::{
    ReportArtifactKind, ReportDetail, ReportEvent, ReportOutcome, ReportPhase, ReportScope,
};
pub use metrics::{MetricValue, ReportMetric};
pub use text::{render_event, TextObserver};

/// A request-local consumer of structured compiler observations.
///
/// Producers query [`enabled`](Self::enabled) before constructing optional
/// detail. Observing an event is deliberately infallible so presentation
/// failures cannot change compilation results.
pub trait ReportObserver {
    fn enabled(&self, detail: ReportDetail) -> bool;

    fn observe(&mut self, event: ReportEvent);
}

/// An observer that disables and discards every report event.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoopObserver;

impl ReportObserver for NoopObserver {
    fn enabled(&self, _detail: ReportDetail) -> bool {
        false
    }

    fn observe(&mut self, _event: ReportEvent) {}
}

/// An in-memory observer for repository tools and typed assertions.
#[derive(Debug)]
pub struct RecordingObserver {
    detail: ReportDetail,
    events: Vec<ReportEvent>,
}

impl RecordingObserver {
    pub fn new(detail: ReportDetail) -> Self {
        Self {
            detail,
            events: Vec::new(),
        }
    }

    pub fn detail(&self) -> ReportDetail {
        self.detail
    }

    pub fn events(&self) -> &[ReportEvent] {
        &self.events
    }

    pub fn into_events(self) -> Vec<ReportEvent> {
        self.events
    }
}

impl ReportObserver for RecordingObserver {
    fn enabled(&self, detail: ReportDetail) -> bool {
        self.detail.includes(detail)
    }

    fn observe(&mut self, event: ReportEvent) {
        if self.detail.includes(ReportDetail::Phases) {
            self.events.push(event);
        }
    }
}

#[cfg(test)]
mod tests;
