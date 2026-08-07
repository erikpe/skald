use super::{diff, escape_bytes, escape_command, escape_path};
use crate::{
    BuildExecution, CompilationIssue, Determinism, ExitExpectation, LeafExecution, MatchMode,
    MatcherLoadFailure, MatcherMismatch, MatcherOutcome, OutputFileMismatch, PlanExecution,
    PlannedLeafKind, ProcessObservation, ProcessTermination, RunExecution, RunMismatch,
    SelectedPlan, StageStatus, StreamComparison,
};
use serde::Serialize;
use std::{collections::BTreeSet, fmt, num::NonZeroUsize, time::Duration};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ReportFormat {
    #[default]
    Human,
    Json,
    Junit,
}

impl std::str::FromStr for ReportFormat {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "human" => Ok(Self::Human),
            "json" => Ok(Self::Json),
            "junit" => Ok(Self::Junit),
            _ => Err(format!(
                "unknown report format {value:?}; expected human, json, or junit"
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ReportOptions {
    show_output: bool,
    slowest: Option<NonZeroUsize>,
}

impl ReportOptions {
    pub fn with_show_output(mut self, show: bool) -> Self {
        self.show_output = show;
        self
    }

    pub fn with_slowest(mut self, count: Option<NonZeroUsize>) -> Self {
        self.slowest = count;
        self
    }

    pub fn show_output(self) -> bool {
        self.show_output
    }

    pub fn slowest(self) -> Option<NonZeroUsize> {
        self.slowest
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Report {
    pub schema_version: u32,
    pub determinism: String,
    pub duration_ms: f64,
    pub counts: ReportCounts,
    pub runtime: Option<StageReport>,
    pub cases: Vec<CaseReport>,
    pub scheduler_failure: Option<SchedulerFailureReport>,
    #[serde(skip)]
    pub(super) options: ReportOptions,
}

#[derive(Debug, Clone, Serialize, Default, PartialEq, Eq)]
pub struct ReportCounts {
    pub specs: usize,
    pub source_tests: usize,
    pub compile_fail_builds: usize,
    pub successful_builds: usize,
    pub named_runs: usize,
    pub leaves_passed: usize,
    pub leaves_failed: usize,
    pub leaves_cancelled: usize,
    pub compiler_processes: usize,
    pub links: usize,
    pub executions: usize,
    pub failures: usize,
    pub cancellations: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CaseReport {
    pub id: String,
    pub spec_id: String,
    pub test_id: String,
    pub build_id: String,
    pub kind: String,
    pub status: String,
    pub duration_ms: f64,
    pub stages: Vec<StageReport>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct StageReport {
    pub stage: String,
    pub status: String,
    pub duration_ms: f64,
    pub artifact_directory: Option<String>,
    pub artifact_retained: Option<bool>,
    pub processes: Vec<ProcessReport>,
    pub failures: Vec<FailureReport>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct ProcessReport {
    pub repetition: usize,
    pub command: String,
    pub working_directory: String,
    pub duration_ms: f64,
    pub termination: Option<String>,
    pub stdout: Option<StreamReport>,
    pub stderr: Option<StreamReport>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct StreamReport {
    pub length: usize,
    pub escaped: String,
    pub policy: Option<String>,
    pub match_offset: Option<usize>,
    pub matchers: Vec<MatcherReport>,
}

/// One ordered matcher result for a captured stream.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MatcherReport {
    pub index: usize,
    pub name: Option<String>,
    pub policy: String,
    pub status: String,
    pub match_offset: Option<usize>,
    pub expected_length: Option<usize>,
    pub expected: Option<String>,
    pub path: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct FailureReport {
    pub kind: String,
    pub message: String,
    pub policy: Option<String>,
    pub expected_length: Option<usize>,
    pub actual_length: Option<usize>,
    pub expected: Option<String>,
    pub actual: Option<String>,
    pub diff: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SchedulerFailureReport {
    pub message: String,
    pub active_nodes: Vec<String>,
    pub pending_nodes: Vec<String>,
}

impl Report {
    pub fn selection_counts(selected: &SelectedPlan<'_>) -> ReportCounts {
        planned_counts(selected)
    }

    pub fn new(
        selected: &SelectedPlan<'_>,
        execution: &PlanExecution,
        determinism: Determinism,
        options: ReportOptions,
    ) -> Self {
        let mut cases = Vec::with_capacity(selected.leaves().len());
        assert_eq!(
            selected.leaves().len(),
            execution.leaves().len(),
            "selected leaves and execution results must have equal lengths"
        );
        for (planned, result) in selected.leaves().iter().zip(execution.leaves()) {
            debug_assert_eq!(planned.id(), result.leaf_id());
            let build = execution
                .builds()
                .iter()
                .find(|build| build.build_id() == planned.build_id())
                .expect("every selected leaf must have a build result");
            let artifact_directory = selected
                .plan()
                .build(planned.build_id())
                .expect("every selected leaf must have a planned build")
                .artifact_directory();
            cases.push(case_report(planned, result, build, artifact_directory));
        }
        let counts = counts(selected, execution);
        let scheduler_failure =
            execution
                .scheduler_failure()
                .map(|failure| SchedulerFailureReport {
                    message: failure.message().to_owned(),
                    active_nodes: failure.active_nodes().to_vec(),
                    pending_nodes: failure.pending_nodes().to_vec(),
                });
        Self {
            schema_version: 1,
            determinism: determinism_name(determinism).to_owned(),
            duration_ms: milliseconds(execution.elapsed()),
            counts,
            runtime: execution.runtime().map(|runtime| StageReport {
                stage: "runtime".to_owned(),
                status: status_name(runtime.status()).to_owned(),
                duration_ms: runtime.process().map_or(0.0, process_ms),
                artifact_directory: Some(escape_path(runtime.archive())),
                artifact_retained: Some(true),
                processes: vec![process_report(runtime.command(), runtime.process(), 1)],
                failures: status_failure(runtime.status()),
            }),
            cases,
            scheduler_failure,
            options,
        }
    }

    pub fn empty(
        selected: &SelectedPlan<'_>,
        determinism: Determinism,
        options: ReportOptions,
    ) -> Self {
        Self {
            schema_version: 1,
            determinism: determinism_name(determinism).to_owned(),
            duration_ms: 0.0,
            counts: planned_counts(selected),
            runtime: None,
            cases: Vec::new(),
            scheduler_failure: None,
            options,
        }
    }

    pub fn passed(&self) -> bool {
        self.scheduler_failure.is_none()
            && self
                .runtime
                .as_ref()
                .is_none_or(|runtime| runtime.status == "passed")
            && self.cases.iter().all(|case| case.status == "passed")
    }
}

fn case_report(
    planned: &&crate::PlannedLeaf,
    leaf: &LeafExecution,
    build: &BuildExecution,
    artifact_directory: &std::path::Path,
) -> CaseReport {
    let mut stages = vec![compilation_stage(build, artifact_directory)];
    if let Some(link) = build.link() {
        stages.push(StageReport {
            stage: "link".to_owned(),
            status: status_name(link.status()).to_owned(),
            duration_ms: link.process().map_or(0.0, process_ms),
            artifact_directory: Some(escape_path(artifact_directory)),
            artifact_retained: Some(true),
            processes: link
                .command()
                .map(|command| vec![process_report(command, link.process(), 1)])
                .unwrap_or_default(),
            failures: status_failure(link.status()),
        });
    }
    for (index, run) in leaf.repetitions().iter().enumerate() {
        let label = if leaf.repetitions().len() == 1 {
            "execution".to_owned()
        } else {
            format!("execution-{}", index + 1)
        };
        let mut failures = run_failures(run);
        if failures.is_empty() && !leaf.status().passed() {
            failures.extend(status_failure(leaf.status()));
        }
        stages.push(StageReport {
            stage: label,
            status: if run.passed() { "passed" } else { "failed" }.to_owned(),
            duration_ms: process_ms(run.observation()),
            artifact_directory: Some(escape_path(run.sandbox())),
            artifact_retained: Some(run.retained()),
            processes: vec![ProcessReport {
                repetition: index + 1,
                command: escape_command(run.command()),
                working_directory: escape_path(run.command().working_directory()),
                duration_ms: process_ms(run.observation()),
                termination: Some(termination(run.observation().termination())),
                stdout: Some(stream_comparison(
                    run.observation().stdout(),
                    run.stdout_comparison(),
                )),
                stderr: Some(stream_comparison(
                    run.observation().stderr(),
                    run.stderr_comparison(),
                )),
            }],
            failures,
        });
    }
    if leaf.repetitions().is_empty() && !matches!(planned.kind(), PlannedLeafKind::Compile(_)) {
        stages.push(StageReport {
            stage: "execution".to_owned(),
            status: status_name(leaf.status()).to_owned(),
            duration_ms: 0.0,
            artifact_directory: Some(escape_path(artifact_directory)),
            artifact_retained: Some(true),
            processes: Vec::new(),
            failures: status_failure(leaf.status()),
        });
    }
    if !leaf.status().passed()
        && !leaf.repetitions().is_empty()
        && leaf.repetitions().iter().all(|run| run.passed())
    {
        stages.push(StageReport {
            stage: "verification".to_owned(),
            status: status_name(leaf.status()).to_owned(),
            duration_ms: 0.0,
            artifact_directory: Some(escape_path(artifact_directory)),
            artifact_retained: Some(true),
            processes: Vec::new(),
            failures: status_failure(leaf.status()),
        });
    }
    CaseReport {
        id: planned.id().to_owned(),
        spec_id: planned.spec_id().to_owned(),
        test_id: planned.test_id().to_owned(),
        build_id: planned.build_id().to_owned(),
        kind: match planned.kind() {
            PlannedLeafKind::Run(_) => "run",
            PlannedLeafKind::Compile(_) => "compile-fail",
        }
        .to_owned(),
        status: status_name(leaf.status()).to_owned(),
        duration_ms: match planned.kind() {
            PlannedLeafKind::Compile(_) => stages
                .iter()
                .filter(|stage| stage.stage == "compile-fail")
                .map(|stage| stage.duration_ms)
                .sum(),
            PlannedLeafKind::Run(_) => stages
                .iter()
                .filter(|stage| stage.stage.starts_with("execution"))
                .map(|stage| stage.duration_ms)
                .sum(),
        },
        stages,
    }
}

fn compilation_stage(build: &BuildExecution, artifact_directory: &std::path::Path) -> StageReport {
    let compilation = build.compilation();
    let processes = compilation
        .observations()
        .iter()
        .enumerate()
        .map(|(index, observation)| {
            let mut report =
                process_report(observation.command(), observation.process(), index + 1);
            if index == 0 {
                if let (Some(process), Some(comparison)) =
                    (observation.process(), compilation.stdout_comparison())
                {
                    report.stdout = Some(stream_comparison(process.stdout(), comparison));
                }
                if let (Some(process), Some(comparison)) =
                    (observation.process(), compilation.stderr_comparison())
                {
                    report.stderr = Some(stream_comparison(process.stderr(), comparison));
                }
            }
            report
        })
        .collect::<Vec<_>>();
    StageReport {
        stage: match compilation.kind() {
            crate::CompilationKind::Success => "compile",
            crate::CompilationKind::CompileFail => "compile-fail",
        }
        .to_owned(),
        status: if compilation.observations().is_empty()
            && matches!(build.status(), StageStatus::Cancelled { .. })
        {
            "cancelled"
        } else if compilation.passed() {
            "passed"
        } else {
            "failed"
        }
        .to_owned(),
        duration_ms: compilation
            .observations()
            .iter()
            .filter_map(|observation| observation.process())
            .map(process_ms)
            .sum(),
        artifact_directory: Some(escape_path(artifact_directory)),
        artifact_retained: Some(true),
        processes,
        failures: if compilation.observations().is_empty()
            && matches!(build.status(), StageStatus::Cancelled { .. })
        {
            status_failure(build.status())
        } else {
            compilation
                .issues()
                .iter()
                .map(|issue| compilation_failure(compilation, issue))
                .collect()
        },
    }
}

fn process_report(
    command: &crate::ProcessCommand,
    observation: Option<&ProcessObservation>,
    repetition: usize,
) -> ProcessReport {
    ProcessReport {
        repetition,
        command: escape_command(command),
        working_directory: escape_path(command.working_directory()),
        duration_ms: observation.map_or(0.0, process_ms),
        termination: observation.map(|process| termination(process.termination())),
        stdout: observation.map(|process| stream(process.stdout(), None, Vec::new())),
        stderr: observation.map(|process| stream(process.stderr(), None, Vec::new())),
    }
}

fn counts(selected: &SelectedPlan<'_>, execution: &PlanExecution) -> ReportCounts {
    let mut result = planned_counts(selected);
    for leaf in execution.leaves() {
        match leaf.status() {
            StageStatus::Passed => result.leaves_passed += 1,
            StageStatus::Failed(_) => result.leaves_failed += 1,
            StageStatus::Cancelled { .. } => result.leaves_cancelled += 1,
        }
        result.executions += leaf.repetitions().len();
    }
    result.compiler_processes = execution
        .builds()
        .iter()
        .map(|build| build.compilation().observations().len())
        .sum();
    result.links = execution
        .builds()
        .iter()
        .filter_map(BuildExecution::link)
        .filter(|link| !matches!(link.status(), StageStatus::Cancelled { .. }))
        .count();
    if let Some(runtime) = execution.runtime() {
        count_status(runtime.status(), &mut result);
    }
    for build in execution.builds() {
        if !build.compilation().passed() {
            if build.compilation().observations().is_empty()
                && matches!(build.status(), StageStatus::Cancelled { .. })
            {
                result.cancellations += 1;
            } else {
                result.failures += 1;
            }
        }
        if let Some(link) = build.link() {
            count_status(link.status(), &mut result);
        }
    }
    for (planned, leaf) in selected.leaves().iter().zip(execution.leaves()) {
        if matches!(planned.kind(), PlannedLeafKind::Run(_)) {
            count_status(leaf.status(), &mut result);
        }
    }
    result
}

fn count_status(status: &StageStatus, counts: &mut ReportCounts) {
    match status {
        StageStatus::Passed => {}
        StageStatus::Failed(_) => counts.failures += 1,
        StageStatus::Cancelled { .. } => counts.cancellations += 1,
    }
}

fn planned_counts(selected: &SelectedPlan<'_>) -> ReportCounts {
    let specs = selected
        .leaves()
        .iter()
        .map(|leaf| leaf.spec_id())
        .collect::<BTreeSet<_>>()
        .len();
    let source_tests = selected
        .leaves()
        .iter()
        .map(|leaf| leaf.test_id())
        .collect::<BTreeSet<_>>()
        .len();
    let compile_fail_builds = selected
        .leaves()
        .iter()
        .filter(|leaf| matches!(leaf.kind(), PlannedLeafKind::Compile(_)))
        .map(|leaf| leaf.build_id())
        .collect::<BTreeSet<_>>()
        .len();
    let successful_builds = selected
        .leaves()
        .iter()
        .filter(|leaf| matches!(leaf.kind(), PlannedLeafKind::Run(_)))
        .map(|leaf| leaf.build_id())
        .collect::<BTreeSet<_>>()
        .len();
    ReportCounts {
        specs,
        source_tests,
        compile_fail_builds,
        successful_builds,
        named_runs: selected
            .leaves()
            .iter()
            .filter(|leaf| matches!(leaf.kind(), PlannedLeafKind::Run(_)))
            .count(),
        ..ReportCounts::default()
    }
}

fn compilation_failure(
    compilation: &crate::CompilationExecution,
    issue: &CompilationIssue,
) -> FailureReport {
    match issue {
        CompilationIssue::Process(message) => plain_failure("compiler-process", message.clone()),
        CompilationIssue::Termination { expected, actual } => plain_failure(
            "compiler-termination",
            format!(
                "expected exit code {expected}, observed {}",
                termination(*actual)
            ),
        ),
        CompilationIssue::Pipe(failure) => plain_failure(
            "compiler-pipe",
            format!("{:?} pipe failed: {}", failure.pipe(), failure.message()),
        ),
        CompilationIssue::StdoutExpectation(mismatch) => matcher_mismatch_failure(
            "stdout",
            mismatch,
            comparison_actual(compilation.stdout_comparison()),
        ),
        CompilationIssue::StderrExpectation(mismatch) => matcher_mismatch_failure(
            "stderr",
            mismatch,
            comparison_actual(compilation.stderr_comparison()),
        ),
        CompilationIssue::StdoutExpectationLoad(failure) => matcher_load_failure("stdout", failure),
        CompilationIssue::StderrExpectationLoad(failure) => matcher_load_failure("stderr", failure),
        CompilationIssue::UnexpectedStdout(bytes) => bytes_failure(
            "unexpected-stdout",
            "successful compilation produced stdout",
            &[],
            bytes,
            Some("exact"),
        ),
        CompilationIssue::UnexpectedStderr(bytes) => bytes_failure(
            "unexpected-stderr",
            "successful compilation produced stderr",
            &[],
            bytes,
            Some("exact"),
        ),
        CompilationIssue::MissingAssembly(path) => plain_failure(
            "missing-assembly",
            format!("compiler did not create {}", path.display()),
        ),
        CompilationIssue::AssemblyRead { path, message } => plain_failure(
            "assembly-read",
            format!("could not read {}: {message}", path.display()),
        ),
        CompilationIssue::NonUtf8Assembly(path) => plain_failure(
            "assembly-encoding",
            format!("assembly {} is not UTF-8", path.display()),
        ),
        CompilationIssue::NondeterministicAssembly => plain_failure(
            "compile-determinism",
            "repeated compiler processes emitted different assembly",
        ),
        CompilationIssue::NondeterministicDiagnostics => plain_failure(
            "compile-determinism",
            "repeated compiler processes produced different diagnostics",
        ),
    }
}

fn run_failures(run: &RunExecution) -> Vec<FailureReport> {
    run.mismatches()
        .iter()
        .map(|mismatch| run_failure(run, mismatch))
        .collect()
}

fn run_failure(run: &RunExecution, mismatch: &RunMismatch) -> FailureReport {
    match mismatch {
        RunMismatch::Exit { expected, actual } => plain_failure(
            "exit",
            format!(
                "expected {}, observed {}",
                expected_exit(*expected),
                termination(*actual)
            ),
        ),
        RunMismatch::Stdout(mismatch) => {
            matcher_mismatch_failure("stdout", mismatch, run.stdout_comparison().actual())
        }
        RunMismatch::Stderr(mismatch) => {
            matcher_mismatch_failure("stderr", mismatch, run.stderr_comparison().actual())
        }
        RunMismatch::StdoutLoad(failure) => matcher_load_failure("stdout", failure),
        RunMismatch::StderrLoad(failure) => matcher_load_failure("stderr", failure),
        RunMismatch::OutputFile(mismatch) => output_file_failure(mismatch),
        RunMismatch::Pipe(failure) => plain_failure(
            "pipe",
            format!("{:?} pipe failed: {}", failure.pipe(), failure.message()),
        ),
    }
}

fn output_file_failure(mismatch: &OutputFileMismatch) -> FailureReport {
    let actual = mismatch.actual().unwrap_or_default();
    bytes_failure(
        "output-file",
        &format!("output file {:?} did not match", mismatch.name()),
        mismatch.expected(),
        actual,
        Some("exact"),
    )
}

fn matcher_mismatch_failure(
    stream_name: &str,
    mismatch: &MatcherMismatch,
    actual: &[u8],
) -> FailureReport {
    let matcher = mismatch.name().map_or_else(
        || format!("matcher {}", mismatch.index()),
        |name| format!("matcher {name:?}"),
    );
    bytes_failure(
        stream_name,
        &format!(
            "{stream_name} {matcher} did not satisfy {} matching",
            mode(mismatch.mode())
        ),
        mismatch.expected(),
        actual,
        Some(mode(mismatch.mode())),
    )
}

fn matcher_load_failure(stream_name: &str, failure: &MatcherLoadFailure) -> FailureReport {
    let matcher = failure.name().map_or_else(
        || format!("matcher {}", failure.index()),
        |name| format!("matcher {name:?}"),
    );
    plain_failure(
        "expectation-load",
        format!("could not load {stream_name} {matcher}: {failure}"),
    )
}

fn comparison_actual(comparison: Option<&StreamComparison>) -> &[u8] {
    comparison.map(StreamComparison::actual).unwrap_or_default()
}

fn bytes_failure(
    kind: &str,
    message: &str,
    expected: &[u8],
    actual: &[u8],
    policy: Option<&str>,
) -> FailureReport {
    FailureReport {
        kind: kind.to_owned(),
        message: message.to_owned(),
        policy: policy.map(str::to_owned),
        expected_length: Some(expected.len()),
        actual_length: Some(actual.len()),
        expected: Some(escape_bytes(expected)),
        actual: Some(escape_bytes(actual)),
        diff: diff(expected, actual),
    }
}

fn plain_failure(kind: &str, message: impl Into<String>) -> FailureReport {
    FailureReport {
        kind: kind.to_owned(),
        message: message.into(),
        policy: None,
        expected_length: None,
        actual_length: None,
        expected: None,
        actual: None,
        diff: None,
    }
}

fn status_failure(status: &StageStatus) -> Vec<FailureReport> {
    match status {
        StageStatus::Passed => Vec::new(),
        StageStatus::Failed(message) => vec![plain_failure("stage", message)],
        StageStatus::Cancelled { dependency } => vec![plain_failure(
            "cancellation",
            format!("cancelled because of {dependency}"),
        )],
    }
}

fn stream_comparison(bytes: &[u8], comparison: &StreamComparison) -> StreamReport {
    if comparison.is_ignored() {
        return stream(bytes, Some(("ignore", None)), Vec::new());
    }
    let matchers = comparison
        .outcomes()
        .iter()
        .map(matcher_report)
        .collect::<Vec<_>>();
    if let [outcome] = comparison.outcomes() {
        return match outcome {
            MatcherOutcome::Matched(result) => stream(
                bytes,
                Some((mode(result.mode()), Some(result.offset()))),
                matchers,
            ),
            MatcherOutcome::Mismatched(result) => {
                stream(bytes, Some((mode(result.mode()), None)), matchers)
            }
            MatcherOutcome::LoadFailed(result) => {
                stream(bytes, Some((mode(result.mode()), None)), matchers)
            }
        };
    }
    stream(bytes, None, matchers)
}

fn matcher_report(outcome: &MatcherOutcome) -> MatcherReport {
    match outcome {
        MatcherOutcome::Matched(result) => MatcherReport {
            index: result.index(),
            name: result.name().map(str::to_owned),
            policy: mode(result.mode()).to_owned(),
            status: "matched".to_owned(),
            match_offset: Some(result.offset()),
            expected_length: Some(result.expected_length()),
            expected: None,
            path: None,
            error: None,
        },
        MatcherOutcome::Mismatched(result) => MatcherReport {
            index: result.index(),
            name: result.name().map(str::to_owned),
            policy: mode(result.mode()).to_owned(),
            status: "mismatched".to_owned(),
            match_offset: None,
            expected_length: Some(result.expected().len()),
            expected: Some(escape_bytes(result.expected())),
            path: None,
            error: None,
        },
        MatcherOutcome::LoadFailed(result) => MatcherReport {
            index: result.index(),
            name: result.name().map(str::to_owned),
            policy: mode(result.mode()).to_owned(),
            status: "load-failed".to_owned(),
            match_offset: None,
            expected_length: None,
            expected: None,
            path: Some(escape_path(result.path())),
            error: Some(result.to_string()),
        },
    }
}

fn stream(
    bytes: &[u8],
    match_data: Option<(&str, Option<usize>)>,
    matchers: Vec<MatcherReport>,
) -> StreamReport {
    StreamReport {
        length: bytes.len(),
        escaped: escape_bytes(bytes),
        policy: match_data.map(|(policy, _)| policy.to_owned()),
        match_offset: match_data.and_then(|(_, offset)| offset),
        matchers,
    }
}

fn expected_exit(expectation: ExitExpectation) -> String {
    match expectation {
        ExitExpectation::Code(code) => format!("exit code {code}"),
        ExitExpectation::Failure => "any nonzero exit code or signal".to_owned(),
    }
}

fn termination(value: ProcessTermination) -> String {
    match value {
        ProcessTermination::Code(code) => format!("exit code {code}"),
        ProcessTermination::Signal(signal) => format!("signal {signal}"),
        ProcessTermination::TimedOut { limit } => {
            format!("timeout after {:.3}s", limit.as_secs_f64())
        }
    }
}

fn mode(value: MatchMode) -> &'static str {
    match value {
        MatchMode::Exact => "exact",
        MatchMode::StartsWith => "starts-with",
        MatchMode::Contains => "contains",
    }
}

fn status_name(status: &StageStatus) -> &'static str {
    match status {
        StageStatus::Passed => "passed",
        StageStatus::Failed(_) => "failed",
        StageStatus::Cancelled { .. } => "cancelled",
    }
}

fn process_ms(process: &ProcessObservation) -> f64 {
    milliseconds(process.elapsed())
}

fn milliseconds(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
}

fn determinism_name(value: Determinism) -> &'static str {
    match value {
        Determinism::Off => "off",
        Determinism::Compile => "compile",
        Determinism::Full => "full",
    }
}

impl fmt::Display for ReportFormat {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Human => "human",
            Self::Json => "json",
            Self::Junit => "junit",
        })
    }
}
