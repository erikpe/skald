use crate::{
    ExitExpectation, PipeFailure, ProcessEnvironment, ProcessObservation, ProcessTermination,
    StreamMatch, StreamMismatch,
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
    inherited_environment: ProcessEnvironment,
    retention: SandboxRetention,
}

impl ExecutionOptions {
    pub fn new(temporary_root: impl Into<PathBuf>) -> Self {
        Self {
            temporary_root: temporary_root.into(),
            default_timeout: Duration::from_secs(10),
            inherited_environment: ProcessEnvironment::new(),
            retention: SandboxRetention::Failures,
        }
    }

    pub fn with_default_timeout(mut self, timeout: Duration) -> Self {
        self.default_timeout = timeout;
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

/// Captured contents of one declared temporary output file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputFileObservation {
    name: String,
    contents: Option<Vec<u8>>,
}

impl OutputFileObservation {
    pub(super) fn new(name: String, contents: Option<Vec<u8>>) -> Self {
        Self { name, contents }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn contents(&self) -> Option<&[u8]> {
        self.contents.as_deref()
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
    Stdout(StreamMismatch),
    Stderr(StreamMismatch),
    OutputFile(OutputFileMismatch),
    Pipe(PipeFailure),
}

/// A complete native execution and all independently observed mismatches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunExecution {
    sandbox: PathBuf,
    retained: bool,
    observation: ProcessObservation,
    stdout_comparison: Result<StreamMatch, StreamMismatch>,
    stderr_comparison: Result<StreamMatch, StreamMismatch>,
    output_files: Vec<OutputFileObservation>,
    mismatches: Vec<RunMismatch>,
}

impl RunExecution {
    pub(super) fn new(
        sandbox: PathBuf,
        retained: bool,
        observation: ProcessObservation,
        stdout_comparison: Result<StreamMatch, StreamMismatch>,
        stderr_comparison: Result<StreamMatch, StreamMismatch>,
        output_files: Vec<OutputFileObservation>,
        mismatches: Vec<RunMismatch>,
    ) -> Self {
        Self {
            sandbox,
            retained,
            observation,
            stdout_comparison,
            stderr_comparison,
            output_files,
            mismatches,
        }
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

    pub fn stdout_comparison(&self) -> &Result<StreamMatch, StreamMismatch> {
        &self.stdout_comparison
    }

    pub fn stderr_comparison(&self) -> &Result<StreamMatch, StreamMismatch> {
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
}
