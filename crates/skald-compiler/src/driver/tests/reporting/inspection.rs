use std::{
    io::{self, Write},
    time::Duration,
};

use crate::{
    backend::Target,
    driver::{
        compile_source_to_assembly, compile_source_to_assembly_observed,
        compile_source_to_assembly_observed_inspected, CompilationError, CompilationInspectors,
    },
    mir::dump_mir,
    passes::{
        static_lifecycle::StaticActivationInspectionLabel, MirPipelineCheckpoint,
        MirPipelineCheckpointLabel,
    },
    reporting::{
        RecordingObserver, ReportDetail, ReportEvent, ReportMetric, ReportPhase, TextObserver,
    },
};

use super::metrics::{count_metric, phase_metrics};

pub(super) fn default_mir_checkpoint_labels() -> [MirPipelineCheckpointLabel; 20] {
    [
        MirPipelineCheckpointLabel::ProofRichInput,
        MirPipelineCheckpointLabel::AfterProofRichPass {
            position: 0,
            pass_name: "dead-pure-definition-elimination",
            occurrence: 0,
        },
        MirPipelineCheckpointLabel::AfterProofRichPass {
            position: 1,
            pass_name: "primitive-constant-folding",
            occurrence: 0,
        },
        MirPipelineCheckpointLabel::AfterProofRichPass {
            position: 2,
            pass_name: "primitive-algebraic-simplification",
            occurrence: 0,
        },
        MirPipelineCheckpointLabel::AfterProofRichPass {
            position: 3,
            pass_name: "primitive-constant-folding",
            occurrence: 1,
        },
        MirPipelineCheckpointLabel::AfterProofRichPass {
            position: 4,
            pass_name: "integer-cast-chain-canonicalization",
            occurrence: 0,
        },
        MirPipelineCheckpointLabel::AfterProofRichPass {
            position: 5,
            pass_name: "checked-integer-constant-folding",
            occurrence: 0,
        },
        MirPipelineCheckpointLabel::AfterProofRichPass {
            position: 6,
            pass_name: "checked-f64-to-integer-constant-folding",
            occurrence: 0,
        },
        MirPipelineCheckpointLabel::AfterProofRichPass {
            position: 7,
            pass_name: "primitive-constant-folding",
            occurrence: 2,
        },
        MirPipelineCheckpointLabel::AfterProofRichPass {
            position: 8,
            pass_name: "dead-pure-definition-elimination",
            occurrence: 1,
        },
        MirPipelineCheckpointLabel::AfterProofRichPass {
            position: 9,
            pass_name: "conservative-cfg-cleanup",
            occurrence: 0,
        },
        MirPipelineCheckpointLabel::AfterProofRichPass {
            position: 10,
            pass_name: "dead-pure-definition-elimination",
            occurrence: 2,
        },
        MirPipelineCheckpointLabel::AfterProofTransitionPass {
            position: 11,
            pass_name: "constant-short-circuit-folding",
            occurrence: 0,
        },
        MirPipelineCheckpointLabel::AfterProofNormalization,
        MirPipelineCheckpointLabel::AfterFinalPass {
            position: 12,
            pass_name: "post-proof-unreachable-block-elimination",
            occurrence: 0,
        },
        MirPipelineCheckpointLabel::AfterFinalPass {
            position: 13,
            pass_name: "dead-normalized-path-activation-cleanup",
            occurrence: 0,
        },
        MirPipelineCheckpointLabel::AfterFinalPass {
            position: 14,
            pass_name: "post-proof-empty-block-forwarding",
            occurrence: 0,
        },
        MirPipelineCheckpointLabel::AfterFinalPass {
            position: 15,
            pass_name: "post-proof-basic-block-merging",
            occurrence: 0,
        },
        MirPipelineCheckpointLabel::AfterFinalPass {
            position: 16,
            pass_name: "whole-world-reachability",
            occurrence: 0,
        },
        MirPipelineCheckpointLabel::Final,
    ]
}

#[test]
fn activation_metrics_and_inspection_keep_distinct_observation_boundaries() {
    let source = "
class State {
    static explicit: i64 = 41;
    static zero: i64;
    static unused: i64 = 7;
    init() {}
}
fn main() -> i64 {
    State.zero = 1;
    return State.explicit + State.zero;
}
";
    let quiet =
        compile_source_to_assembly("activation-observation.ska", source, Target::X86_64SysV)
            .unwrap();
    let mut observer = RecordingObserver::new(ReportDetail::Details);
    let mut inspections = Vec::new();
    let mut inspector =
        |inspection: crate::passes::static_lifecycle::StaticActivationInspection<'_>| {
            inspections.push((
                inspection.label(),
                inspection.planned().activation_statistics(),
                inspection.activation_dump(),
            ));
        };

    let inspected = compile_source_to_assembly_observed_inspected(
        "activation-observation.ska",
        source,
        Target::X86_64SysV,
        &mut observer,
        CompilationInspectors::new().with_static_activation(&mut inspector),
    )
    .unwrap();

    assert_eq!(inspected.assembly, quiet.assembly);
    assert!(inspected.report.diagnostics.is_empty());
    assert_eq!(inspections.len(), 1);
    let (label, statistics, dump) = &inspections[0];
    assert_eq!(*label, StaticActivationInspectionLabel::VerifiedPlanning);
    assert_eq!(statistics.declared_fields(), 3);
    assert_eq!(statistics.active_fields(), 2);
    assert_eq!(statistics.inactive_fields(), 1);
    assert_eq!(statistics.active_explicit_fields(), 1);
    assert_eq!(statistics.active_zero_default_fields(), 1);
    assert_eq!(statistics.inactive_explicit_fields(), 1);
    assert!(dump.contains("State.unused"));
    assert!(dump.contains("  ActivationOrder\n"));
    assert!(dump.contains("  ShutdownOrder\n"));

    let metrics = phase_metrics(observer.events(), ReportPhase::StaticLifecyclePlanning);
    assert_eq!(count_metric(metrics, "declared static fields"), Some(3));
    assert_eq!(count_metric(metrics, "active static fields"), Some(2));
    assert_eq!(count_metric(metrics, "inactive static fields"), Some(1));
    assert_eq!(
        count_metric(metrics, "active explicit static fields"),
        Some(1)
    );
    assert_eq!(
        count_metric(metrics, "active zero-default static fields"),
        Some(1)
    );
    assert_eq!(
        count_metric(metrics, "inactive explicit static fields"),
        Some(1)
    );
    assert!(count_metric(metrics, "activation execution nodes").is_some());
    assert!(count_metric(metrics, "activation edges").is_some());
    assert!(count_metric(metrics, "conservative activation targets").is_some());
    assert!(observer.events().iter().all(|event| match event {
        ReportEvent::PhaseFinished { metrics, .. } => metrics
            .iter()
            .all(|metric| !metric.name().contains("witness")),
        _ => true,
    }));
}

#[test]
fn mir_only_inspection_preserves_artifacts_reports_and_reporting() {
    let path = "mir-inspection-parity.ska";
    let source = concat!(
        "fn choose(left: bool, right: bool) -> bool { return left && right; }\n",
        "fn main() -> i64 {\n",
        "  if (choose(true, true)) { return 42; }\n",
        "  return 0;\n",
        "}\n",
    );
    let mut ordinary_observer = RecordingObserver::new(ReportDetail::Details);
    let ordinary = compile_source_to_assembly_observed(
        path,
        source,
        Target::X86_64SysV,
        &mut ordinary_observer,
    )
    .unwrap();

    let mut inspected_observer = RecordingObserver::new(ReportDetail::Details);
    let mut checkpoints = Vec::new();
    let mut inspector = |checkpoint: crate::passes::MirPipelineCheckpoint<'_>| {
        let (is_final, dump) = match checkpoint {
            MirPipelineCheckpoint::ProofRich(checkpoint) => {
                (false, dump_mir(checkpoint.verified().program()))
            }
            MirPipelineCheckpoint::Final(checkpoint) => {
                (true, dump_mir(checkpoint.verified().program()))
            }
        };
        checkpoints.push((checkpoint.label(), is_final, dump));
    };
    let inspected = compile_source_to_assembly_observed_inspected(
        path,
        source,
        Target::X86_64SysV,
        &mut inspected_observer,
        CompilationInspectors::new().with_mir_pipeline(&mut inspector),
    )
    .unwrap();

    assert_eq!(inspected.assembly, ordinary.assembly);
    assert_eq!(
        inspected.report.diagnostics.len(),
        ordinary.report.diagnostics.len()
    );
    assert_eq!(
        checkpoints
            .iter()
            .map(|(label, _, _)| *label)
            .collect::<Vec<_>>(),
        default_mir_checkpoint_labels()
    );
    assert!(checkpoints
        .iter()
        .filter(|(_, is_final, _)| !is_final)
        .all(|(_, _, dump)| dump.contains("path-condition <path-condition>")));
    assert!(checkpoints
        .iter()
        .filter(|(_, is_final, _)| *is_final)
        .all(|(_, _, dump)| {
            dump.contains("normalized-path-activation <normalized-path-activation>")
                && !dump.contains("path-condition <path-condition>")
        }));
    let metrics = phase_metrics(inspected_observer.events(), ReportPhase::MirPipeline);
    assert_eq!(
        metrics[..9],
        [
            ReportMetric::count("verification executions", 2),
            ReportMetric::count("normalization executions", 1),
            ReportMetric::count("path-condition records consumed", 1),
            ReportMetric::count("logical-expression records consumed", 1),
            ReportMetric::count("path reads lowered", 1),
            ReportMetric::count("activation storage declarations reclassified", 1),
            ReportMetric::count("normalization changed callables", 1),
            ReportMetric::count("proof-protected blocks released", 6),
            ReportMetric::count("pass executions", 17),
        ]
    );
    assert_eq!(
        without_elapsed(inspected_observer.events()),
        without_elapsed(ordinary_observer.events())
    );
}

#[test]
fn report_writer_failure_does_not_block_activation_inspection_or_compilation() {
    let mut observer = TextObserver::new(FailingReportWriter, ReportDetail::Details);
    let mut activation_labels = Vec::new();
    let mut activation_inspector =
        |inspection: crate::passes::static_lifecycle::StaticActivationInspection<'_>| {
            activation_labels.push(inspection.label());
        };
    let mut mir_labels = Vec::new();
    let mut mir_inspector = |checkpoint: MirPipelineCheckpoint<'_>| {
        mir_labels.push(checkpoint.label());
    };

    let artifact = compile_source_to_assembly_observed_inspected(
        "report-failure-inspection.ska",
        "fn main() -> i64 { return 42; }",
        Target::X86_64SysV,
        &mut observer,
        CompilationInspectors::new()
            .with_static_activation(&mut activation_inspector)
            .with_mir_pipeline(&mut mir_inspector),
    )
    .unwrap();

    assert!(artifact.assembly.contains("mov rax, 42"));
    assert_eq!(
        activation_labels,
        [StaticActivationInspectionLabel::VerifiedPlanning]
    );
    assert_eq!(mir_labels, default_mir_checkpoint_labels());
    assert_eq!(observer.error().unwrap().kind(), io::ErrorKind::BrokenPipe);
}

#[test]
fn inactive_initializer_errors_remain_source_diagnostics_without_inspection() {
    let invalid = "
class State { static unused: i64 = true; init() {} }
fn main() -> i64 { return 0; }
";
    let mut observer = RecordingObserver::new(ReportDetail::Details);
    let mut activation_inspections = 0;
    let mut activation_inspector =
        |_: crate::passes::static_lifecycle::StaticActivationInspection<'_>| {
            activation_inspections += 1;
        };
    let mut mir_inspections = 0;
    let mut mir_inspector = |_: crate::passes::MirPipelineCheckpoint<'_>| {
        mir_inspections += 1;
    };

    let result = compile_source_to_assembly_observed_inspected(
        "inactive-error.ska",
        invalid,
        Target::X86_64SysV,
        &mut observer,
        CompilationInspectors::new()
            .with_static_activation(&mut activation_inspector)
            .with_mir_pipeline(&mut mir_inspector),
    );

    let Err(CompilationError::Diagnostics(report)) = result else {
        panic!("inactive initializer must retain its ordinary source error");
    };
    assert_eq!(activation_inspections, 0);
    assert_eq!(mir_inspections, 0);
    assert!(report.diagnostics.has_errors());
    assert!(observer.events().iter().all(|event| !matches!(
        event,
        ReportEvent::PhaseStarted {
            phase: ReportPhase::StaticLifecyclePlanning,
        }
    )));
}

struct FailingReportWriter;

impl Write for FailingReportWriter {
    fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
        Err(io::Error::new(
            io::ErrorKind::BrokenPipe,
            "report sink closed",
        ))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn without_elapsed(events: &[ReportEvent]) -> Vec<ReportEvent> {
    events
        .iter()
        .cloned()
        .map(|mut event| {
            match &mut event {
                ReportEvent::PhaseFinished { elapsed, .. }
                | ReportEvent::RunFinished { elapsed, .. } => *elapsed = Duration::ZERO,
                _ => {}
            }
            event
        })
        .collect()
}
