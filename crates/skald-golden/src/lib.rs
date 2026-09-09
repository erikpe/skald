//! Spec-driven golden-test runner infrastructure.
//!
//! The crate is repository tooling. Its library owns reusable parsing,
//! planning, execution, and reporting responsibilities; the companion binary
//! remains a thin process entry point.

/// Maximum stdout or stderr bytes retained per process by default.
pub const DEFAULT_PROCESS_CAPTURE_LIMIT: usize = 4 * 1024 * 1024;

/// Maximum bytes loaded from one generated or expected output file by default.
pub const DEFAULT_OUTPUT_FILE_LIMIT: usize = 32 * 1024 * 1024;

mod cli;
mod compile;
mod discovery;
mod execute;
mod expectation;
mod plan;
mod process;
mod report;
mod selection;
mod spec;

pub use cli::run_cli;
pub use compile::{
    locate_compiler, CompilationExecution, CompilationIssue, CompilationKind, CompilerConfig,
    CompilerLocationError, CompilerObservation, Determinism,
};
pub use execute::{
    allowlisted_environment, execute_parallel, execute_run, execute_sequential, BuildExecution,
    ExecutionError, ExecutionOptions, LeafExecution, LinkExecution, OutputFileMismatch,
    OutputFileObservation, OutputFileOverflow, OutputFileOverflowKind, PlanExecution, RunExecution,
    RunMismatch, RuntimeExecution, RuntimePreparation, SandboxRetention, SchedulerFailure,
    SchedulerOptions, SequentialExecution, SequentialOptions, StageOptions, StageStatus,
};
pub use expectation::{
    compare_exit, compare_matchers, compare_stream, decode_arguments, load_bytes,
    EmptyStreamMatcherSet, ExpectationError, MatcherLoadFailure, MatcherMatch, MatcherMismatch,
    MatcherOutcome, StreamComparison, StreamMatcher, StreamMatcherSet,
};
pub use plan::{
    build_plan, PlanError, PlannedBuild, PlannedLeaf, PlannedLeafKind, PlannedRun, PlannedSpec,
    PlannedTest, ResolvedArgs, ResolvedByteSource, ResolvedCompileExpectation, ResolvedInputFile,
    ResolvedOutputFile, ResolvedRunExpectation, ResolvedStreamExpectation,
    ResolvedWorkingDirectory, TestPlan,
};
pub use process::{
    run_process, PipeFailure, ProcessCaptureOverflow, ProcessCommand, ProcessEnvironment,
    ProcessError, ProcessObservation, ProcessPipe, ProcessTermination,
};
pub use report::{
    render as render_report, CaseReport, FailureReport, MatcherReport, ProcessReport, Report,
    ReportCounts, ReportFormat, ReportOptions, SchedulerFailureReport, StageReport, StreamReport,
};
pub use selection::{select, SelectedPlan, SelectionError, SelectionOptions};
pub use spec::{
    parse_config, parse_spec, ArgSource, ByteSource, CompileExpectation, CompileFailTest,
    ExitExpectation, InputFile, MatchMode, OutputFileExpectation, RepositoryConfig, Run,
    RunExpectation, RunTest, SchemaVersion, Spec, SpecError, StreamExpectation, Test, TestKind,
    Variant, WorkingDirectory,
};
