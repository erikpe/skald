use std::{fs, panic::AssertUnwindSafe};

use crate::{
    backend::{emit_assembly, BackendInput, Target},
    driver::{
        compile_request_to_assembly_observed, compile_source_to_assembly_observed,
        CompilationError, EntrySelector,
    },
    identity::{ClassId, FieldId, ModuleId},
    mir::{
        MirClassDeclaration, MirClassDeclarationTable, MirCopyCapability, MirDestructionPlan,
        MirFieldDeclaration, MirType,
    },
    passes::{run_mir_pipeline, verify_final_mir, VerifiedFinalMirProgram},
    reporting::{
        MetricValue, RecordingObserver, ReportDetail, ReportEvent, ReportMetric, ReportOutcome,
        ReportPhase,
    },
    test_support::{lower_source_to_final_mir, TemporaryDirectory},
};

use super::{
    metrics::{metric_names, phase_metrics},
    phases::{assert_observation, completed, SINGLETON_SUCCESS_PHASES},
    request::request,
};

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
            "rejected-interface.ska",
            concat!(
                "interface View {}\n",
                "interface Consumer<T> { fn consume(value: T) -> unit; }\n",
                "class Implementation implements Consumer<View> {\n",
                "  fn consume(value: View) -> unit {}\n",
                "  virtual fn read() -> i64 { return 1; }\n",
                "}\n",
                "fn main() -> i64 { return 0; }\n",
            ),
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
    let malformed_pipeline = mir_with_missing_terminator();
    let mut mir_observer = RecordingObserver::new(ReportDetail::Phases);
    let result = crate::driver::observation::observe_phase(
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

    let unsupported_backend = verified_mir_with_recursive_inline_layout();
    let mut backend_observer = RecordingObserver::new(ReportDetail::Phases);
    let result = crate::driver::observation::observe_phase(
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
fn phase_observation_does_not_convert_panics_into_compilation_failures() {
    let mut observer = RecordingObserver::new(ReportDetail::Phases);
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        crate::driver::observation::observe_phase(
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

fn mir_with_missing_terminator() -> crate::mir::MirProgram {
    let mut mir = lower_source_to_final_mir("fn main() -> i64 { return 0; }");
    mir.definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap()
        .body
        .blocks[0]
        .terminator = None;
    mir
}

fn verified_mir_with_recursive_inline_layout() -> VerifiedFinalMirProgram {
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
