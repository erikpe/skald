use std::{fs, panic::AssertUnwindSafe, path::PathBuf, thread};

use crate::{
    backend::{emit_assembly, BackendInput},
    passes::run_mir_pipeline,
    reporting::{
        RecordingObserver, ReportDetail, ReportEvent, ReportOutcome, ReportPhase, ReportScope,
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
}

#[test]
fn request_success_observes_loading_and_the_shared_compiler_pipeline() {
    let workspace = TemporaryDirectory::new("observed-request").unwrap();
    let root = workspace.join("modules");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("app.ska"), "fn main() -> i64 { return 42; }\n").unwrap();
    let request = request(
        &workspace,
        root,
        EntrySelector::Module("app".parse().unwrap()),
    );
    let mut observer = RecordingObserver::new(ReportDetail::Details);

    let artifact = compile_request_to_assembly_observed(&request, &mut observer).unwrap();

    assert!(artifact.report.diagnostics.is_empty());
    assert!(artifact.assembly.contains("mov rax, 42"));
    assert_observation(
        observer.events(),
        &completed(&REQUEST_SUCCESS_PHASES),
        ReportOutcome::Completed,
    );
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
            "fn main() -> i64 { return 0; }\n",
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
}

#[test]
fn malformed_mir_and_backend_errors_receive_failed_phase_outcomes() {
    let mut malformed_pipeline = malformed_final_mir();
    let mut mir_observer = RecordingObserver::new(ReportDetail::Phases);
    let result = super::super::pipeline::observe_phase(
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

    malformed_pipeline = malformed_final_mir();
    let mut backend_observer = RecordingObserver::new(ReportDetail::Phases);
    let result = super::super::pipeline::observe_phase(
        &mut backend_observer,
        ReportPhase::BackendEmission,
        || {
            emit_assembly(
                Target::X86_64SysV,
                BackendInput::without_runtime_trace(&malformed_pipeline),
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
        super::super::pipeline::observe_phase(
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

fn assert_observation(
    events: &[ReportEvent],
    expected: &[(ReportPhase, ReportOutcome)],
    run_outcome: ReportOutcome,
) {
    assert_eq!(events.len(), expected.len() * 2 + 1, "{events:#?}");
    for (index, (phase, outcome)) in expected.iter().copied().enumerate() {
        let offset = index * 2;
        assert_eq!(events[offset], ReportEvent::PhaseStarted { phase });
        let ReportEvent::PhaseFinished {
            phase: finished,
            outcome: actual,
            metrics,
            ..
        } = &events[offset + 1]
        else {
            panic!("phase start was not followed by a finish: {events:#?}");
        };
        assert_eq!(*finished, phase);
        assert_eq!(*actual, outcome);
        assert!(
            metrics.is_empty(),
            "metrics belong to the next roadmap task"
        );
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
