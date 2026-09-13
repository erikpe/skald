use std::thread;

use crate::{
    backend::Target,
    diagnostics::render_diagnostics,
    driver::{compile_source_to_assembly, compile_source_to_assembly_observed, CompilationError},
    reporting::{RecordingObserver, ReportDetail, ReportOutcome},
};

use super::phases::{assert_observation, completed, SINGLETON_SUCCESS_PHASES};

#[test]
fn observation_preserves_success_artifacts_and_failure_diagnostics() {
    let source = concat!(
        "fn choose(left: bool, right: bool) -> bool { return left && right; }\n",
        "fn main() -> i64 {\n",
        "  if (choose(true, true)) { return 42; }\n",
        "  return 0;\n",
        "}\n",
    );
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
