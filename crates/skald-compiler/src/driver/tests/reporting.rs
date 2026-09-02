use std::{
    fs,
    io::{self, Write},
    panic::AssertUnwindSafe,
    path::PathBuf,
    thread,
};

use crate::{
    backend::{emit_assembly, BackendInput},
    identity::{ClassId, FieldId, ModuleId},
    mir::{
        MirClassDeclaration, MirClassDeclarationTable, MirCopyCapability, MirDestructionPlan,
        MirFieldDeclaration, MirType,
    },
    passes::{
        run_mir_pipeline, static_lifecycle::StaticActivationInspectionLabel, verify_final_mir,
        VerifiedFinalMirProgram,
    },
    reporting::{
        MetricValue, RecordingObserver, ReportDetail, ReportEvent, ReportMetric, ReportModuleStage,
        ReportOutcome, ReportPhase, ReportScope, TextObserver,
    },
    test_support::lower_source_to_final_mir,
};

use super::*;

const SINGLETON_SUCCESS_PHASES: [ReportPhase; 11] = [
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
];

const REQUEST_SUCCESS_PHASES: [ReportPhase; 11] = [
    ReportPhase::ProviderNormalization,
    ReportPhase::ModuleLoading,
    ReportPhase::Resolution,
    ReportPhase::TypeChecking,
    ReportPhase::PreliminaryMirLowering,
    ReportPhase::PreliminaryMirVerification,
    ReportPhase::StaticLifecyclePlanning,
    ReportPhase::PlannedMirVerification,
    ReportPhase::StaticLifecycleSynthesis,
    ReportPhase::MirPipeline,
    ReportPhase::BackendEmission,
];

#[test]
fn singleton_success_observes_every_owned_phase_and_compilation_total() {
    let mut observer = RecordingObserver::new(ReportDetail::Phases);
    let artifact = compile_source_to_assembly_observed(
        "observed.ska",
        "fn main() -> i64 { return 42; }",
        Target::X86_64SysV,
        &mut observer,
    )
    .unwrap();

    assert!(artifact.report.diagnostics.is_empty());
    assert!(artifact.assembly.contains("mov rax, 42"));
    assert_observation(
        observer.events(),
        &completed(&SINGLETON_SUCCESS_PHASES),
        ReportOutcome::Completed,
    );
    assert!(observer.events().iter().all(|event| !matches!(
        event,
        ReportEvent::PhaseFinished { metrics, .. } if !metrics.is_empty()
    )));
}

#[test]
fn request_success_observes_loading_and_the_shared_compiler_pipeline() {
    let workspace = TemporaryDirectory::new("observed-request").unwrap();
    let root = workspace.join("modules");
    fs::create_dir_all(&root).unwrap();
    let source = "fn main() -> i64 { return 42; }\n";
    fs::write(root.join("app.ska"), source).unwrap();
    let request = request(
        &workspace,
        root,
        EntrySelector::Module("app".parse().unwrap()),
    );
    let mut observer = RecordingObserver::new(ReportDetail::Trace);
    let mut inspection_labels = Vec::new();
    let mut inspector =
        |inspection: crate::passes::static_lifecycle::StaticActivationInspection<'_>| {
            inspection_labels.push(inspection.label());
        };

    let artifact =
        compile_request_to_assembly_observed_inspected(&request, &mut observer, &mut inspector)
            .unwrap();

    assert!(artifact.report.diagnostics.is_empty());
    assert!(artifact.assembly.contains("mov rax, 42"));
    assert_eq!(
        inspection_labels,
        [StaticActivationInspectionLabel::VerifiedPlanning]
    );
    assert_observation(
        observer.events(),
        &completed(&REQUEST_SUCCESS_PHASES),
        ReportOutcome::Completed,
    );
    assert_eq!(
        observer
            .events()
            .iter()
            .filter_map(|event| match event {
                ReportEvent::MirPassFinished { occurrence } => Some((
                    occurrence.position(),
                    occurrence.name(),
                    occurrence.occurrence(),
                )),
                _ => None,
            })
            .collect::<Vec<_>>(),
        [
            (0, "dead-pure-definition-elimination", 0),
            (1, "primitive-constant-folding", 0),
            (2, "primitive-algebraic-simplification", 0),
            (3, "primitive-constant-folding", 1),
            (4, "dead-pure-definition-elimination", 1),
            (5, "conservative-cfg-cleanup", 0),
            (6, "dead-pure-definition-elimination", 2),
            (7, "whole-world-reachability", 0),
        ]
    );
    let tokens = u64::try_from(crate::test_support::lex_source(source).2.tokens.len()).unwrap();
    assert_eq!(
        observer
            .events()
            .iter()
            .filter(|event| matches!(event, ReportEvent::ModuleParsed { .. }))
            .cloned()
            .collect::<Vec<_>>(),
        vec![
            ReportEvent::ModuleParsed {
                module: "app".to_owned(),
                stage: ReportModuleStage::Discovery,
                tokens,
                outcome: ReportOutcome::Completed,
            },
            ReportEvent::ModuleParsed {
                module: "app".to_owned(),
                stage: ReportModuleStage::Final,
                tokens,
                outcome: ReportOutcome::Completed,
            },
        ]
    );
    let loading_start = observer
        .events()
        .iter()
        .position(|event| {
            matches!(
                event,
                ReportEvent::PhaseStarted {
                    phase: ReportPhase::ModuleLoading,
                }
            )
        })
        .unwrap();
    let loading_finish = observer
        .events()
        .iter()
        .position(|event| {
            matches!(
                event,
                ReportEvent::PhaseFinished {
                    phase: ReportPhase::ModuleLoading,
                    ..
                }
            )
        })
        .unwrap();
    assert!(observer.events()[loading_start + 1..loading_finish]
        .iter()
        .all(|event| matches!(event, ReportEvent::ModuleParsed { .. })));
    assert_eq!(
        phase_metrics(observer.events(), ReportPhase::ModuleLoading),
        &[
            ReportMetric::count("reached modules", 1),
            ReportMetric::count("source reads", 1),
            ReportMetric::bytes("source bytes", source.len() as u64),
            ReportMetric::count("discovery lex executions", 1),
            ReportMetric::count("discovery parse executions", 1),
            ReportMetric::count("discovery tokens", tokens),
            ReportMetric::count("final lex executions", 1),
            ReportMetric::count("final parse executions", 1),
            ReportMetric::count("final tokens", tokens),
        ]
    );
}

#[test]
fn details_publish_deterministic_phase_owned_metrics() {
    let source = "fn main() -> i64 { return 42; }";
    let mut observer = RecordingObserver::new(ReportDetail::Details);
    let artifact = compile_source_to_assembly_observed(
        "metrics.ska",
        source,
        Target::X86_64SysV,
        &mut observer,
    )
    .unwrap();
    let tokens = u64::try_from(crate::test_support::lex_source(source).2.tokens.len()).unwrap();

    assert_eq!(
        phase_metrics(observer.events(), ReportPhase::Lexing),
        &[
            ReportMetric::count("lex executions", 1),
            ReportMetric::bytes("source bytes", source.len() as u64),
            ReportMetric::count("tokens", tokens),
            ReportMetric::count("diagnostics", 0),
            ReportMetric::count("warnings", 0),
            ReportMetric::count("errors", 0),
        ]
    );
    assert_eq!(
        phase_metrics(observer.events(), ReportPhase::Resolution),
        &[
            ReportMetric::count("modules", 1),
            ReportMetric::count("function declarations", 1),
            ReportMetric::count("function definitions", 1),
            ReportMetric::count("class declarations", 0),
            ReportMetric::count("class definitions", 0),
            ReportMetric::count("interface declarations", 0),
            ReportMetric::count("diagnostics", 0),
            ReportMetric::count("warnings", 0),
            ReportMetric::count("errors", 0),
        ]
    );
    assert_eq!(
        phase_metrics(observer.events(), ReportPhase::TypeChecking),
        &[
            ReportMetric::count("modules", 1),
            ReportMetric::count("function definitions", 1),
            ReportMetric::count("class definitions", 0),
            ReportMetric::count("diagnostics", 0),
            ReportMetric::count("warnings", 0),
            ReportMetric::count("errors", 0),
        ]
    );
    let preliminary = phase_metrics(observer.events(), ReportPhase::PreliminaryMirLowering);
    assert_eq!(preliminary[0], ReportMetric::count("definitions", 1));
    assert_eq!(preliminary[1], ReportMetric::count("blocks", 1));
    assert_eq!(
        metric_names(preliminary),
        ["definitions", "blocks", "instructions"]
    );
    assert_eq!(
        phase_metrics(observer.events(), ReportPhase::StaticLifecyclePlanning,),
        &[
            ReportMetric::count("effect summaries", 1),
            ReportMetric::count("dependencies", 0),
            ReportMetric::count("declared static fields", 0),
            ReportMetric::count("active static fields", 0),
            ReportMetric::count("inactive static fields", 0),
            ReportMetric::count("active explicit static fields", 0),
            ReportMetric::count("active zero-default static fields", 0),
            ReportMetric::count("inactive explicit static fields", 0),
            ReportMetric::count("activation execution nodes", 1),
            ReportMetric::count("activation edges", 0),
            ReportMetric::count("conservative activation targets", 0),
            ReportMetric::count("activation fields", 0),
            ReportMetric::count("shutdown fields", 0),
            ReportMetric::count("static initializers", 0),
        ]
    );
    let synthesis = phase_metrics(observer.events(), ReportPhase::StaticLifecycleSynthesis);
    assert_eq!(synthesis[0], ReportMetric::count("definitions", 1));
    assert_eq!(synthesis[1], ReportMetric::count("blocks", 1));
    let pipeline = phase_metrics(observer.events(), ReportPhase::MirPipeline);
    assert_eq!(
        pipeline[..7],
        [
            ReportMetric::count("verification executions", 1),
            ReportMetric::count("pass executions", 8),
            ReportMetric::count("processed callables", 8),
            ReportMetric::count("changed callables", 0),
            ReportMetric::count("retained MIR entities", 0),
            ReportMetric::count("inserted MIR entities", 0),
            ReportMetric::count("removed MIR entities", 0),
        ]
    );
    assert_eq!(
        pipeline[7],
        ReportMetric::pass_count(
            "dead-pure-definition-elimination",
            "removed assignment instructions",
            0,
        )
    );
    assert_eq!(
        pipeline[8],
        ReportMetric::pass_count(
            "dead-pure-definition-elimination",
            "removed value declarations",
            0,
        )
    );
    let folding = |name| ReportMetric::pass_count("primitive-constant-folding", name, 0);
    assert_eq!(
        pipeline[9..13],
        [
            folding("folded unary assignments"),
            folding("folded binary assignments"),
            folding("folded comparison assignments"),
            folding("folded cast assignments"),
        ]
    );
    let algebra = |name| ReportMetric::pass_count("primitive-algebraic-simplification", name, 0);
    assert_eq!(
        pipeline[13..18],
        [
            algebra("constant-result rewrites"),
            algebra("forwarded value uses"),
            algebra("removed assignment instructions"),
            algebra("removed value declarations"),
            algebra("rejected protected-use candidates"),
        ]
    );
    let cfg = |name| ReportMetric::pass_count("conservative-cfg-cleanup", name, 0);
    assert_eq!(
        pipeline[18..23],
        [
            cfg("folded constant branches"),
            cfg("folded same-target branches"),
            cfg("removed blocks"),
            cfg("removed value declarations"),
            cfg("retained protected unreachable blocks"),
        ]
    );
    let reachability =
        |name, value| ReportMetric::pass_count("whole-world-reachability", name, value);
    assert_eq!(
        pipeline[23..44],
        [
            reachability("examined definitions", 1),
            reachability("examined function definitions", 1),
            reachability("examined static-initializer definitions", 0),
            reachability("examined member definitions", 0),
            reachability("reachable definitions", 1),
            reachability("reachable function definitions", 1),
            reachability("reachable static-initializer definitions", 0),
            reachability("reachable member definitions", 0),
            reachability("removed definitions", 0),
            reachability("removed function definitions", 0),
            reachability("removed static-initializer definitions", 0),
            reachability("removed member definitions", 0),
            reachability("whole-program roots", 1),
            reachability("reachable execution nodes", 1),
            reachability("reachable callables", 1),
            reachability("dependency edges", 0),
            reachability("runtime entity targets", 0),
            reachability("virtual dispatch families", 0),
            reachability("interface dispatch requirements", 0),
            reachability("function-value signatures", 0),
            reachability("function-value targets", 0),
        ]
    );
    assert_eq!(pipeline[44], ReportMetric::count("definitions", 1));
    assert_eq!(pipeline[45], ReportMetric::count("blocks", 1));
    assert_eq!(
        phase_metrics(observer.events(), ReportPhase::BackendEmission),
        &[
            ReportMetric::bytes("assembly bytes", artifact.assembly.len() as u64),
            ReportMetric::count("assembly lines", artifact.assembly.lines().count() as u64,),
        ]
    );
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
        &mut inspector,
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
fn report_writer_failure_does_not_block_activation_inspection_or_compilation() {
    let mut observer = TextObserver::new(FailingReportWriter, ReportDetail::Details);
    let mut labels = Vec::new();
    let mut inspector =
        |inspection: crate::passes::static_lifecycle::StaticActivationInspection<'_>| {
            labels.push(inspection.label());
        };

    let artifact = compile_source_to_assembly_observed_inspected(
        "report-failure-inspection.ska",
        "fn main() -> i64 { return 42; }",
        Target::X86_64SysV,
        &mut observer,
        &mut inspector,
    )
    .unwrap();

    assert!(artifact.assembly.contains("mov rax, 42"));
    assert_eq!(labels, [StaticActivationInspectionLabel::VerifiedPlanning]);
    assert_eq!(observer.error().unwrap().kind(), io::ErrorKind::BrokenPipe);
}

#[test]
fn inactive_initializer_errors_remain_source_diagnostics_without_inspection() {
    let invalid = "
class State { static unused: i64 = true; init() {} }
fn main() -> i64 { return 0; }
";
    let mut observer = RecordingObserver::new(ReportDetail::Details);
    let mut inspections = 0;
    let mut inspector = |_: crate::passes::static_lifecycle::StaticActivationInspection<'_>| {
        inspections += 1;
    };

    let result = compile_source_to_assembly_observed_inspected(
        "inactive-error.ska",
        invalid,
        Target::X86_64SysV,
        &mut observer,
        &mut inspector,
    );

    let Err(CompilationError::Diagnostics(report)) = result else {
        panic!("inactive initializer must retain its ordinary source error");
    };
    assert_eq!(inspections, 0);
    assert!(report.diagnostics.has_errors());
    assert!(observer.events().iter().all(|event| !matches!(
        event,
        ReportEvent::PhaseStarted {
            phase: ReportPhase::StaticLifecyclePlanning,
        }
    )));
}

#[test]
fn provider_and_loading_failures_stop_at_their_existing_boundaries() {
    let workspace = TemporaryDirectory::new("observed-request-failure").unwrap();
    let invalid = request(
        &workspace,
        workspace.join("missing-root"),
        EntrySelector::Module("app".parse().unwrap()),
    );
    let mut provider_observer = RecordingObserver::new(ReportDetail::Trace);
    assert!(matches!(
        compile_request_to_assembly_observed(&invalid, &mut provider_observer),
        Err(CompilationError::ProviderConfiguration(_))
    ));
    assert_observation(
        provider_observer.events(),
        &[(ReportPhase::ProviderNormalization, ReportOutcome::Failed)],
        ReportOutcome::Failed,
    );

    let root = workspace.join("modules");
    fs::create_dir(&root).unwrap();
    let missing = request(
        &workspace,
        root,
        EntrySelector::Module("missing".parse().unwrap()),
    );
    let mut loading_observer = RecordingObserver::new(ReportDetail::Trace);
    assert!(matches!(
        compile_request_to_assembly_observed(&missing, &mut loading_observer),
        Err(CompilationError::Diagnostics(_))
    ));
    assert_observation(
        loading_observer.events(),
        &[
            (ReportPhase::ProviderNormalization, ReportOutcome::Completed),
            (ReportPhase::ModuleLoading, ReportOutcome::Failed),
        ],
        ReportOutcome::Failed,
    );
    let loading_metrics = phase_metrics(loading_observer.events(), ReportPhase::ModuleLoading);
    assert_eq!(
        loading_metrics[0],
        ReportMetric::count("reached modules", 0)
    );
    assert_eq!(loading_metrics[1], ReportMetric::count("source reads", 0));
    assert_eq!(
        metric_names(&loading_metrics[loading_metrics.len() - 3..]),
        ["diagnostics", "warnings", "errors"]
    );
    assert_ne!(
        loading_metrics.last().unwrap().value(),
        MetricValue::Count(0)
    );
}

#[test]
fn singleton_source_failures_stop_after_the_owning_frontend_phase() {
    let cases = [
        (
            "lex.ska",
            "@",
            vec![(ReportPhase::Lexing, ReportOutcome::Failed)],
        ),
        (
            "parse.ska",
            "fn main(",
            vec![
                (ReportPhase::Lexing, ReportOutcome::Completed),
                (ReportPhase::Parsing, ReportOutcome::Failed),
            ],
        ),
        (
            "resolve.ska",
            "fn main() -> i64 { return missing(); }",
            vec![
                (ReportPhase::Lexing, ReportOutcome::Completed),
                (ReportPhase::Parsing, ReportOutcome::Completed),
                (ReportPhase::Resolution, ReportOutcome::Failed),
            ],
        ),
        (
            "typeck.ska",
            "fn main() -> i64 { return true; }",
            vec![
                (ReportPhase::Lexing, ReportOutcome::Completed),
                (ReportPhase::Parsing, ReportOutcome::Completed),
                (ReportPhase::Resolution, ReportOutcome::Completed),
                (ReportPhase::TypeChecking, ReportOutcome::Failed),
            ],
        ),
    ];

    for (path, source, expected) in cases {
        let mut observer = RecordingObserver::new(ReportDetail::Trace);
        assert!(matches!(
            compile_source_to_assembly_observed(path, source, Target::X86_64SysV, &mut observer,),
            Err(CompilationError::Diagnostics(_))
        ));
        assert_observation(observer.events(), &expected, ReportOutcome::Failed);
        let failed_phase = expected.last().unwrap().0;
        assert!(phase_metrics(observer.events(), failed_phase)
            .iter()
            .any(|metric| metric.name() == "errors" && metric.value() != MetricValue::Count(0)));
    }
}

#[test]
fn lifecycle_planning_diagnostics_stop_before_planned_mir_verification() {
    let mut observer = RecordingObserver::new(ReportDetail::Trace);
    let result = compile_source_to_assembly_observed(
        "static-cycle.ska",
        concat!(
            "fn read_left() -> i64 { return State.left; }\n",
            "fn read_right() -> i64 { return State.right; }\n",
            "class State {\n",
            "  static left: i64 = read_right();\n",
            "  static right: i64 = read_left();\n",
            "  init() {}\n",
            "}\n",
            "fn main() -> i64 { return State.left; }\n",
        ),
        Target::X86_64SysV,
        &mut observer,
    );

    let Err(CompilationError::Diagnostics(report)) = result else {
        panic!("expected lifecycle diagnostics");
    };
    assert_eq!(report.diagnostics.len(), 1);
    let mut expected = completed(&SINGLETON_SUCCESS_PHASES[..6]);
    expected.push((ReportPhase::StaticLifecyclePlanning, ReportOutcome::Failed));
    assert_observation(observer.events(), &expected, ReportOutcome::Failed);
    assert_eq!(
        metric_names(phase_metrics(
            observer.events(),
            ReportPhase::StaticLifecyclePlanning,
        )),
        ["dependencies", "diagnostics", "warnings", "errors"]
    );
}

#[test]
fn malformed_mir_and_backend_errors_receive_failed_phase_outcomes() {
    let malformed_pipeline = malformed_final_mir();
    let mut mir_observer = RecordingObserver::new(ReportDetail::Phases);
    let result = super::super::observation::observe_phase(
        &mut mir_observer,
        ReportPhase::MirPipeline,
        || run_mir_pipeline(malformed_pipeline),
        result_phase_outcome,
    );
    assert!(result.is_err());
    assert_phase_pair(
        mir_observer.events(),
        ReportPhase::MirPipeline,
        ReportOutcome::Failed,
    );

    let unsupported_backend = unsupported_backend_mir();
    let mut backend_observer = RecordingObserver::new(ReportDetail::Phases);
    let result = super::super::observation::observe_phase(
        &mut backend_observer,
        ReportPhase::BackendEmission,
        || {
            emit_assembly(
                Target::X86_64SysV,
                BackendInput::without_runtime_trace(&unsupported_backend),
            )
        },
        result_phase_outcome,
    );
    assert!(result.is_err());
    assert_phase_pair(
        backend_observer.events(),
        ReportPhase::BackendEmission,
        ReportOutcome::Failed,
    );
}

#[test]
fn observation_preserves_success_artifacts_and_failure_diagnostics() {
    let source = "fn main() -> i64 { return 42; }";
    let quiet = compile_source_to_assembly("same.ska", source, Target::X86_64SysV).unwrap();
    let mut observer = RecordingObserver::new(ReportDetail::Trace);
    let observed =
        compile_source_to_assembly_observed("same.ska", source, Target::X86_64SysV, &mut observer)
            .unwrap();
    assert_eq!(observed.assembly, quiet.assembly);
    assert_eq!(observed.report.sources.len(), quiet.report.sources.len());
    assert_eq!(
        render_diagnostics(&observed.report.sources, &observed.report.diagnostics),
        render_diagnostics(&quiet.report.sources, &quiet.report.diagnostics)
    );

    let mut disabled = RecordingObserver::new(ReportDetail::Off);
    let disabled_artifact =
        compile_source_to_assembly_observed("same.ska", source, Target::X86_64SysV, &mut disabled)
            .unwrap();
    assert_eq!(disabled_artifact.assembly, quiet.assembly);
    assert!(disabled.events().is_empty());

    let invalid = "fn main() -> i64 { return true; }";
    let quiet = compile_source_to_assembly("same-error.ska", invalid, Target::X86_64SysV);
    let mut observer = RecordingObserver::new(ReportDetail::Trace);
    let observed = compile_source_to_assembly_observed(
        "same-error.ska",
        invalid,
        Target::X86_64SysV,
        &mut observer,
    );
    let (Err(CompilationError::Diagnostics(quiet)), Err(CompilationError::Diagnostics(observed))) =
        (quiet, observed)
    else {
        panic!("both paths must retain source diagnostics");
    };
    assert_eq!(
        render_diagnostics(&observed.sources, &observed.diagnostics),
        render_diagnostics(&quiet.sources, &quiet.diagnostics)
    );
}

#[test]
fn independent_observers_do_not_share_events_across_repeated_or_parallel_calls() {
    let compile = |value| {
        let mut observer = RecordingObserver::new(ReportDetail::Phases);
        let artifact = compile_source_to_assembly_observed(
            format!("parallel-{value}.ska"),
            format!("fn main() -> i64 {{ return {value}; }}"),
            Target::X86_64SysV,
            &mut observer,
        )
        .unwrap();
        (artifact.assembly, observer.into_events())
    };

    let first = compile(1);
    let second = compile(2);
    assert_ne!(first.0, second.0);
    assert_observation(
        &first.1,
        &completed(&SINGLETON_SUCCESS_PHASES),
        ReportOutcome::Completed,
    );
    assert_observation(
        &second.1,
        &completed(&SINGLETON_SUCCESS_PHASES),
        ReportOutcome::Completed,
    );

    let handles: Vec<_> = (3..7)
        .map(|value| thread::spawn(move || compile(value)))
        .collect();
    for handle in handles {
        let (_, events) = handle.join().unwrap();
        assert_observation(
            &events,
            &completed(&SINGLETON_SUCCESS_PHASES),
            ReportOutcome::Completed,
        );
    }
}

#[test]
fn phase_observation_does_not_convert_panics_into_compilation_failures() {
    let mut observer = RecordingObserver::new(ReportDetail::Phases);
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        super::super::observation::observe_phase(
            &mut observer,
            ReportPhase::Resolution,
            || panic!("internal defect"),
            |_: &()| ReportOutcome::Completed,
        )
    }));

    assert!(result.is_err());
    assert_eq!(
        observer.events(),
        &[ReportEvent::PhaseStarted {
            phase: ReportPhase::Resolution,
        }]
    );
}

fn request(
    workspace: &TemporaryDirectory,
    root: PathBuf,
    entry: EntrySelector,
) -> CompilationRequest {
    CompilationRequest::new(
        entry,
        vec![root],
        StandardLibrarySelection::Disabled,
        Target::X86_64SysV,
        ArtifactOptions::new(ArtifactKind::Assembly, None),
        CompilationEnvironment::new(workspace.path().to_owned(), workspace.join("unused-std")),
    )
}

fn completed(phases: &[ReportPhase]) -> Vec<(ReportPhase, ReportOutcome)> {
    phases
        .iter()
        .copied()
        .map(|phase| (phase, ReportOutcome::Completed))
        .collect()
}

fn phase_metrics(events: &[ReportEvent], phase: ReportPhase) -> &[ReportMetric] {
    events
        .iter()
        .find_map(|event| match event {
            ReportEvent::PhaseFinished {
                phase: finished,
                metrics,
                ..
            } if *finished == phase => Some(metrics.as_slice()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing finished phase {phase:?}"))
}

fn metric_names(metrics: &[ReportMetric]) -> Vec<&'static str> {
    metrics.iter().map(ReportMetric::name).collect()
}

fn count_metric(metrics: &[ReportMetric], name: &str) -> Option<u64> {
    metrics.iter().find_map(|metric| {
        (metric.name() == name).then(|| match metric.value() {
            MetricValue::Count(value) => value,
            MetricValue::Bytes(_) => panic!("`{name}` must be a count metric"),
        })
    })
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

fn assert_observation(
    events: &[ReportEvent],
    expected: &[(ReportPhase, ReportOutcome)],
    run_outcome: ReportOutcome,
) {
    let phase_events = events
        .iter()
        .filter(|event| {
            matches!(
                event,
                ReportEvent::PhaseStarted { .. } | ReportEvent::PhaseFinished { .. }
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(phase_events.len(), expected.len() * 2, "{events:#?}");
    for (index, (phase, outcome)) in expected.iter().copied().enumerate() {
        let offset = index * 2;
        assert_eq!(*phase_events[offset], ReportEvent::PhaseStarted { phase });
        let ReportEvent::PhaseFinished {
            phase: finished,
            outcome: actual,
            ..
        } = phase_events[offset + 1]
        else {
            panic!("phase start was not followed by a finish: {events:#?}");
        };
        assert_eq!(*finished, phase);
        assert_eq!(*actual, outcome);
    }
    assert!(matches!(
        events.last(),
        Some(ReportEvent::RunFinished {
            scope: ReportScope::Compilation,
            outcome,
            ..
        }) if *outcome == run_outcome
    ));
}

fn assert_phase_pair(events: &[ReportEvent], phase: ReportPhase, outcome: ReportOutcome) {
    assert_eq!(events.len(), 2);
    assert_eq!(events[0], ReportEvent::PhaseStarted { phase });
    assert!(matches!(
        &events[1],
        ReportEvent::PhaseFinished {
            phase: finished,
            outcome: actual,
            metrics,
            ..
        } if *finished == phase && *actual == outcome && metrics.is_empty()
    ));
}

fn result_phase_outcome<T, E>(result: &Result<T, E>) -> ReportOutcome {
    if result.is_ok() {
        ReportOutcome::Completed
    } else {
        ReportOutcome::Failed
    }
}

fn malformed_final_mir() -> crate::mir::MirProgram {
    let mut mir = lower_source_to_final_mir("fn main() -> i64 { return 0; }");
    mir.definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap()
        .body
        .blocks[0]
        .terminator = None;
    mir
}

fn unsupported_backend_mir() -> VerifiedFinalMirProgram {
    let mut mir = lower_source_to_final_mir("fn main() -> i64 { return 0; }");
    let class = ClassId::new(0);
    let field = FieldId::new(class, 0);
    mir.classes = MirClassDeclarationTable::new(vec![MirClassDeclaration {
        id: class,
        module: ModuleId::new(0),
        name: "Recursive".to_owned(),
        direct_base: None,
        conformances: vec![],
        static_fields: vec![],
        fields: vec![MirFieldDeclaration {
            id: field,
            cell_span: None,
            final_span: None,
            name: "self".to_owned(),
            ty: MirType::Class(class),
            span: mir.span,
        }],
        initializers: vec![],
        copy_constructor_declaration: None,
        copy_constructor: MirCopyCapability::Unavailable,
        copy_assignment_declaration: None,
        copy_assignment: MirCopyCapability::Unavailable,
        destruction: MirDestructionPlan::new(None, &[field]),
        methods: vec![],
        span: mir.span,
    }]);
    verify_final_mir(mir).expect("target-independent MIR permits recursive inline layout")
}
