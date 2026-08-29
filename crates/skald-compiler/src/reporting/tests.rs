use std::{
    io::{self, Write},
    path::PathBuf,
    time::Duration,
};

use super::*;

const ALL_PHASES: [ReportPhase; 15] = [
    ReportPhase::ProviderNormalization,
    ReportPhase::ModuleLoading,
    ReportPhase::Lexing,
    ReportPhase::Parsing,
    ReportPhase::Resolution,
    ReportPhase::TypeChecking,
    ReportPhase::PreliminaryMirLowering,
    ReportPhase::PreliminaryMirVerification,
    ReportPhase::StaticLifecyclePlanning,
    ReportPhase::PlannedMirVerification,
    ReportPhase::StaticLifecycleSynthesis,
    ReportPhase::MirPipeline,
    ReportPhase::BackendEmission,
    ReportPhase::HostLinking,
    ReportPhase::ArtifactPublication,
];

#[test]
fn detail_levels_are_ordered_and_off_enables_nothing() {
    let off = RecordingObserver::new(ReportDetail::Off);
    assert!(!off.enabled(ReportDetail::Off));
    assert!(!off.enabled(ReportDetail::Phases));

    let phases = RecordingObserver::new(ReportDetail::Phases);
    assert!(!phases.enabled(ReportDetail::Off));
    assert!(phases.enabled(ReportDetail::Phases));
    assert!(!phases.enabled(ReportDetail::Details));

    let details = RecordingObserver::new(ReportDetail::Details);
    assert!(details.enabled(ReportDetail::Phases));
    assert!(details.enabled(ReportDetail::Details));
    assert!(!details.enabled(ReportDetail::Trace));

    let trace = RecordingObserver::new(ReportDetail::Trace);
    assert!(trace.enabled(ReportDetail::Phases));
    assert!(trace.enabled(ReportDetail::Details));
    assert!(trace.enabled(ReportDetail::Trace));
}

#[test]
fn no_op_and_disabled_recording_observers_discard_owned_events() {
    let event = ReportEvent::PhaseStarted {
        phase: ReportPhase::Resolution,
    };
    let mut noop = NoopObserver;
    assert!(!noop.enabled(ReportDetail::Phases));
    noop.observe(event.clone());

    let mut recording = RecordingObserver::new(ReportDetail::Off);
    recording.observe(event);
    assert!(recording.events().is_empty());
}

#[test]
fn recording_observer_owns_exact_events_in_emission_order() {
    let expected = vec![
        ReportEvent::PhaseStarted {
            phase: ReportPhase::Resolution,
        },
        ReportEvent::PhaseFinished {
            phase: ReportPhase::Resolution,
            elapsed: Duration::from_micros(1_250),
            outcome: ReportOutcome::Completed,
            metrics: vec![
                ReportMetric::count("modules", 2),
                ReportMetric::bytes("source bytes", 4_096),
            ],
        },
    ];
    let mut observer = RecordingObserver::new(ReportDetail::Details);
    for event in expected.clone() {
        observer.observe(event);
    }

    assert_eq!(observer.detail(), ReportDetail::Details);
    assert_eq!(observer.events(), expected);
    assert_eq!(observer.into_events(), expected);
}

#[test]
fn metric_constructors_preserve_name_value_unit_and_owner_order() {
    let metrics = [
        ReportMetric::bytes("source bytes", 1),
        ReportMetric::count("modules", 3),
    ];
    assert_eq!(metrics[0].name(), "source bytes");
    assert_eq!(metrics[0].value(), MetricValue::Bytes(1));
    assert_eq!(metrics[1].name(), "modules");
    assert_eq!(metrics[1].value(), MetricValue::Count(3));

    let event = ReportEvent::PhaseFinished {
        phase: ReportPhase::ModuleLoading,
        elapsed: Duration::ZERO,
        outcome: ReportOutcome::Completed,
        metrics: metrics.to_vec(),
    };
    assert_eq!(
        render_event(&event, ReportDetail::Details),
        concat!(
            "skac: phase: module loading completed in 0.000 ms\n",
            "skac: stats: source bytes: 1 byte\n",
            "skac: stats: modules: 3\n",
        )
    );
}

#[test]
fn phase_rendering_covers_every_typed_identity_and_detail_policy() {
    for phase in ALL_PHASES {
        let started = ReportEvent::PhaseStarted { phase };
        let finished = ReportEvent::PhaseFinished {
            phase,
            elapsed: Duration::from_nanos(12_345_500),
            outcome: ReportOutcome::Failed,
            metrics: vec![ReportMetric::count("operations", 7)],
        };

        assert!(render_event(&started, ReportDetail::Phases).starts_with("skac: phase: "));
        let phases = render_event(&finished, ReportDetail::Phases);
        assert!(phases.ends_with(" failed\n"));
        assert!(!phases.contains(" ms"));
        assert!(!phases.contains("stats"));

        let details = render_event(&finished, ReportDetail::Details);
        assert!(details.contains(" failed in 12.346 ms\n"));
        assert!(details.ends_with("skac: stats: operations: 7\n"));
        assert_eq!(details, render_event(&finished, ReportDetail::Trace));
    }

    assert_eq!(
        render_event(
            &ReportEvent::PhaseStarted {
                phase: ReportPhase::Resolution,
            },
            ReportDetail::Off,
        ),
        ""
    );
}

#[test]
fn run_and_artifact_rendering_covers_every_scope_outcome_and_kind() {
    for scope in [ReportScope::Compilation, ReportScope::Driver] {
        for outcome in [ReportOutcome::Completed, ReportOutcome::Failed] {
            let rendered = render_event(
                &ReportEvent::RunFinished {
                    scope,
                    elapsed: Duration::from_millis(8),
                    outcome,
                },
                ReportDetail::Details,
            );
            assert!(rendered.starts_with("skac: run: "));
            assert!(rendered.ends_with(" in 8.000 ms\n"));
        }
    }

    for kind in [
        ReportArtifactKind::Assembly,
        ReportArtifactKind::Executable,
        ReportArtifactKind::Dump,
    ] {
        let rendered = render_event(
            &ReportEvent::ArtifactPublished {
                kind,
                path: PathBuf::from("build/mód.s"),
            },
            ReportDetail::Phases,
        );
        assert!(rendered.starts_with("skac: artifact: "));
        assert!(rendered.ends_with(" build/mód.s\n"));
        assert_eq!(rendered.matches('\n').count(), 1);
    }
}

#[cfg(unix)]
#[test]
fn artifact_rendering_uses_native_lossy_display_for_non_utf8_paths() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt};

    let rendered = render_event(
        &ReportEvent::ArtifactPublished {
            kind: ReportArtifactKind::Assembly,
            path: PathBuf::from(OsString::from_vec(b"build/bad\xff.s".to_vec())),
        },
        ReportDetail::Phases,
    );
    assert_eq!(rendered, "skac: artifact: assembly build/bad�.s\n");
}

#[test]
fn text_observer_completes_short_writes() {
    let mut observer = TextObserver::new(ShortWriter::new(3), ReportDetail::Phases);
    observer.observe(ReportEvent::PhaseStarted {
        phase: ReportPhase::Resolution,
    });

    assert!(observer.error().is_none());
    let (writer, error) = observer.into_parts();
    assert!(error.is_none());
    assert_eq!(writer.bytes, b"skac: phase: resolution started\n");
    assert!(writer.write_calls > 1);
}

#[test]
fn text_observer_retains_first_error_and_suppresses_later_writes() {
    let writer = FailingWriter::new(5, "first failure");
    let mut observer = TextObserver::new(writer, ReportDetail::Trace);
    observer.observe(ReportEvent::PhaseStarted {
        phase: ReportPhase::Resolution,
    });

    assert_eq!(observer.error().unwrap().to_string(), "first failure");
    assert!(!observer.enabled(ReportDetail::Phases));
    observer.observe(ReportEvent::PhaseStarted {
        phase: ReportPhase::TypeChecking,
    });

    let (writer, error) = observer.into_parts();
    assert_eq!(error.unwrap().to_string(), "first failure");
    assert_eq!(writer.write_calls, 2);
}

struct ShortWriter {
    maximum: usize,
    bytes: Vec<u8>,
    write_calls: usize,
}

impl ShortWriter {
    fn new(maximum: usize) -> Self {
        Self {
            maximum,
            bytes: Vec::new(),
            write_calls: 0,
        }
    }
}

impl Write for ShortWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.write_calls += 1;
        let length = buffer.len().min(self.maximum);
        self.bytes.extend_from_slice(&buffer[..length]);
        Ok(length)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct FailingWriter {
    remaining: usize,
    message: &'static str,
    write_calls: usize,
}

impl FailingWriter {
    fn new(remaining: usize, message: &'static str) -> Self {
        Self {
            remaining,
            message,
            write_calls: 0,
        }
    }
}

impl Write for FailingWriter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        self.write_calls += 1;
        if self.remaining == 0 {
            return Err(io::Error::new(io::ErrorKind::BrokenPipe, self.message));
        }
        let length = buffer.len().min(self.remaining);
        self.remaining -= length;
        Ok(length)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
