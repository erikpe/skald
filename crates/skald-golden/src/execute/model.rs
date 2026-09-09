use crate::process::DEFAULT_TIMEOUT;
use crate::{
    ExitExpectation, MatcherLoadFailure, MatcherMismatch, PipeFailure, ProcessCaptureOverflow,
    ProcessCommand, ProcessEnvironment, ProcessObservation, ProcessTermination, StreamComparison,
    DEFAULT_OUTPUT_FILE_LIMIT, DEFAULT_PROCESS_CAPTURE_LIMIT,
};
use std::{path::PathBuf, time::Duration};

/// Whether a passing run's private directory should be retained.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SandboxRetention {
    #[default]
    Failures,
    All,
}

/// Process-independent controls for one native run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionOptions {
    temporary_root: PathBuf,
    default_timeout: Duration,
    capture_limit: usize,
    output_file_limit: usize,
    inherited_environment: ProcessEnvironment,
    retention: SandboxRetention,
}

impl ExecutionOptions {
    pub fn new(temporary_root: impl Into<PathBuf>) -> Self {
        Self {
            temporary_root: temporary_root.into(),
            default_timeout: DEFAULT_TIMEOUT,
            capture_limit: DEFAULT_PROCESS_CAPTURE_LIMIT,
            output_file_limit: DEFAULT_OUTPUT_FILE_LIMIT,
            inherited_environment: ProcessEnvironment::new(),
            retention: SandboxRetention::Failures,
        }
    }

    pub fn with_default_timeout(mut self, timeout: Duration) -> Self {
        self.default_timeout = timeout;
        self
    }

    pub fn with_capture_limit(mut self, limit: usize) -> Self {
        self.capture_limit = limit;
        self
    }

    pub fn with_output_file_limit(mut self, limit: usize) -> Self {
        self.output_file_limit = limit;
        self
    }

    pub fn with_inherited_environment(mut self, environment: ProcessEnvironment) -> Self {
        self.inherited_environment = environment;
        self
    }

    pub fn with_retention(mut self, retention: SandboxRetention) -> Self {
        self.retention = retention;
        self
    }

    pub fn temporary_root(&self) -> &std::path::Path {
        &self.temporary_root
    }

    pub fn default_timeout(&self) -> Duration {
        self.default_timeout
    }

    pub fn capture_limit(&self) -> usize {
        self.capture_limit
    }

    pub fn output_file_limit(&self) -> usize {
        self.output_file_limit
    }

    pub fn inherited_environment(&self) -> &ProcessEnvironment {
        &self.inherited_environment
    }

    pub fn retention(&self) -> SandboxRetention {
        self.retention
    }
}

/// An exact temporary output-file mismatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputFileMismatch {
    name: String,
    expected: Vec<u8>,
    actual: Option<Vec<u8>>,
}

/// Retained bytes and limit status for one declared temporary output file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputFileObservation {
    name: String,
    contents: Option<Vec<u8>>,
    overflow: Option<OutputFileOverflow>,
}

impl OutputFileObservation {
    pub(super) fn new(
        name: String,
        contents: Option<Vec<u8>>,
        overflow: Option<OutputFileOverflow>,
    ) -> Self {
        Self {
            name,
            contents,
            overflow,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn contents(&self) -> Option<&[u8]> {
        self.contents.as_deref()
    }

    pub fn overflow(&self) -> Option<&OutputFileOverflow> {
        self.overflow.as_ref()
    }
}

/// Which side of an exact output-file comparison exceeded its byte limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFileOverflowKind {
    Expectation,
    Observation,
}

/// A declared output file whose complete bytes were not loaded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputFileOverflow {
    name: String,
    kind: OutputFileOverflowKind,
    limit: usize,
}

impl OutputFileOverflow {
    pub(super) fn new(name: String, kind: OutputFileOverflowKind, limit: usize) -> Self {
        Self { name, kind, limit }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn kind(&self) -> OutputFileOverflowKind {
        self.kind
    }

    pub fn limit(&self) -> usize {
        self.limit
    }
}

impl OutputFileMismatch {
    pub(super) fn new(name: String, expected: Vec<u8>, actual: Option<Vec<u8>>) -> Self {
        Self {
            name,
            expected,
            actual,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn expected(&self) -> &[u8] {
        &self.expected
    }

    pub fn actual(&self) -> Option<&[u8]> {
        self.actual.as_deref()
    }
}

/// One independently checkable mismatch from a native process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunMismatch {
    Exit {
        expected: ExitExpectation,
        actual: ProcessTermination,
    },
    Stdout(MatcherMismatch),
    Stderr(MatcherMismatch),
    StdoutLoad(MatcherLoadFailure),
    StderrLoad(MatcherLoadFailure),
    CaptureOverflow(ProcessCaptureOverflow),
    OutputFile(OutputFileMismatch),
    OutputFileOverflow(OutputFileOverflow),
    Pipe(PipeFailure),
}

/// A complete native execution and all independently observed mismatches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunExecution {
    command: ProcessCommand,
    sandbox: PathBuf,
    retained: bool,
    observation: ProcessObservation,
    stdout_comparison: StreamComparison,
    stderr_comparison: StreamComparison,
    output_files: Vec<OutputFileObservation>,
    mismatches: Vec<RunMismatch>,
}

pub(super) struct RunExecutionParts {
    pub(super) command: ProcessCommand,
    pub(super) sandbox: PathBuf,
    pub(super) retained: bool,
    pub(super) observation: ProcessObservation,
    pub(super) stdout_comparison: StreamComparison,
    pub(super) stderr_comparison: StreamComparison,
    pub(super) output_files: Vec<OutputFileObservation>,
    pub(super) mismatches: Vec<RunMismatch>,
}

impl RunExecution {
    pub(super) fn from_parts(parts: RunExecutionParts) -> Self {
        Self {
            command: parts.command,
            sandbox: parts.sandbox,
            retained: parts.retained,
            observation: parts.observation,
            stdout_comparison: parts.stdout_comparison,
            stderr_comparison: parts.stderr_comparison,
            output_files: parts.output_files,
            mismatches: parts.mismatches,
        }
    }

    pub fn command(&self) -> &ProcessCommand {
        &self.command
    }

    pub fn sandbox(&self) -> &std::path::Path {
        &self.sandbox
    }

    pub fn retained(&self) -> bool {
        self.retained
    }

    pub fn observation(&self) -> &ProcessObservation {
        &self.observation
    }

    pub fn stdout_comparison(&self) -> &StreamComparison {
        &self.stdout_comparison
    }

    pub fn stderr_comparison(&self) -> &StreamComparison {
        &self.stderr_comparison
    }

    pub fn output_files(&self) -> &[OutputFileObservation] {
        &self.output_files
    }

    pub fn mismatches(&self) -> &[RunMismatch] {
        &self.mismatches
    }

    pub fn passed(&self) -> bool {
        self.mismatches.is_empty()
    }

    pub(super) fn mark_removed(&mut self) {
        self.retained = false;
    }
}
