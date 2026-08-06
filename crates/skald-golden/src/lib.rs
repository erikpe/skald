//! Spec-driven golden-test runner infrastructure.
//!
//! The crate is repository tooling. Its library owns reusable parsing,
//! planning, execution, and reporting responsibilities; the companion binary
//! remains a thin process entry point.

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
    allowlisted_environment, execute_run, execute_sequential, BuildExecution, ExecutionError,
    ExecutionOptions, LeafExecution, LinkExecution, OutputFileMismatch, OutputFileObservation,
    RunExecution, RunMismatch, RuntimeExecution, RuntimePreparation, SandboxRetention,
    SequentialExecution, SequentialOptions, StageStatus,
};
pub use expectation::{
    compare_exit, compare_stream, decode_arguments, load_bytes, ExpectationError, StreamMatch,
    StreamMismatch,
};
pub use plan::{
    build_plan, PlanError, PlannedBuild, PlannedLeaf, PlannedLeafKind, PlannedRun, PlannedSpec,
    PlannedTest, ResolvedArgs, ResolvedByteSource, ResolvedCompileExpectation, ResolvedInputFile,
    ResolvedOutputFile, ResolvedRunExpectation, ResolvedStreamExpectation,
    ResolvedWorkingDirectory, TestPlan,
};
pub use process::{
    run_process, PipeFailure, ProcessCommand, ProcessEnvironment, ProcessError, ProcessObservation,
    ProcessPipe, ProcessTermination,
};
pub use selection::{select, SelectedPlan, SelectionError, SelectionOptions};
pub use spec::{
    parse_config, parse_spec, ArgSource, ByteSource, CompileExpectation, CompileFailTest,
    ExitExpectation, InputFile, MatchMode, OutputFileExpectation, RepositoryConfig, Run,
    RunExpectation, RunTest, SchemaVersion, Spec, SpecError, StreamExpectation, Test, TestKind,
    Variant, WorkingDirectory,
};
