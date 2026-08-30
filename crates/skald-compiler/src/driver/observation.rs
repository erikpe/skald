//! Generic driver-owned phase observation mechanics.

use std::time::Instant;

use crate::reporting::{
    ReportDetail, ReportEvent, ReportMetric, ReportObserver, ReportOutcome, ReportPhase,
    ReportScope,
};

mod mir_pipeline;

pub(super) use mir_pipeline::observe_mir_pipeline;

/// Runs one scoped operation and emits its total only when phase reporting is enabled.
pub(super) fn observe_run<T>(
    observer: &mut dyn ReportObserver,
    scope: ReportScope,
    operation: impl FnOnce(&mut dyn ReportObserver) -> T,
    outcome: impl FnOnce(&T) -> ReportOutcome,
) -> T {
    let started = observer.enabled(ReportDetail::Phases).then(Instant::now);
    let result = operation(observer);
    if let Some(started) = started {
        observer.observe(ReportEvent::RunFinished {
            scope,
            elapsed: started.elapsed(),
            outcome: outcome(&result),
        });
    }
    result
}

pub(super) fn observe_phase<T>(
    observer: &mut dyn ReportObserver,
    phase: ReportPhase,
    operation: impl FnOnce() -> T,
    outcome: impl FnOnce(&T) -> ReportOutcome,
) -> T {
    observe_phase_with_metrics(observer, phase, operation, outcome, |_, _| Vec::new())
}

pub(super) fn observe_phase_with_metrics<T>(
    observer: &mut dyn ReportObserver,
    phase: ReportPhase,
    operation: impl FnOnce() -> T,
    outcome: impl FnOnce(&T) -> ReportOutcome,
    metrics: impl FnOnce(&T, &mut dyn ReportObserver) -> Vec<ReportMetric>,
) -> T {
    if !observer.enabled(ReportDetail::Phases) {
        return operation();
    }

    let started = Instant::now();
    observer.observe(ReportEvent::PhaseStarted { phase });
    let result = operation();
    let outcome = outcome(&result);
    let metrics = if observer.enabled(ReportDetail::Details) {
        metrics(&result, observer)
    } else {
        Vec::new()
    };
    observer.observe(ReportEvent::PhaseFinished {
        phase,
        elapsed: started.elapsed(),
        outcome,
        metrics,
    });
    result
}

#[cfg(test)]
mod tests {
    use crate::reporting::{RecordingObserver, ReportDetail};

    use super::*;

    #[test]
    fn optional_metric_construction_runs_only_for_details_and_trace() {
        for detail in [ReportDetail::Off, ReportDetail::Phases] {
            let mut observer = RecordingObserver::new(detail);
            let result = observe_phase_with_metrics(
                &mut observer,
                ReportPhase::Resolution,
                || 42,
                |_| ReportOutcome::Completed,
                |_, _| panic!("metric construction must remain disabled"),
            );
            assert_eq!(result, 42);
        }

        for detail in [ReportDetail::Details, ReportDetail::Trace] {
            let mut observer = RecordingObserver::new(detail);
            let result = observe_phase_with_metrics(
                &mut observer,
                ReportPhase::Resolution,
                || 42,
                |_| ReportOutcome::Completed,
                |_, _| vec![ReportMetric::count("answers", 1)],
            );
            assert_eq!(result, 42);
            assert!(matches!(
                observer.events().last(),
                Some(ReportEvent::PhaseFinished { metrics, .. })
                    if metrics == &[ReportMetric::count("answers", 1)]
            ));
        }
    }

    #[test]
    fn disabled_observation_skips_run_timing_and_event_construction() {
        let mut observer = RejectingDisabledObserver;

        let result = observe_run(
            &mut observer,
            ReportScope::Driver,
            |_| 42,
            |_| ReportOutcome::Completed,
        );

        assert_eq!(result, 42);
    }

    struct RejectingDisabledObserver;

    impl ReportObserver for RejectingDisabledObserver {
        fn enabled(&self, _detail: ReportDetail) -> bool {
            false
        }

        fn observe(&mut self, _event: ReportEvent) {
            panic!("disabled observation must not construct or emit events");
        }
    }
}
