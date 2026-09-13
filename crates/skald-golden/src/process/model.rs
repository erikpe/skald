use super::DEFAULT_TIMEOUT;
use crate::DEFAULT_PROCESS_CAPTURE_LIMIT;
use std::{collections::BTreeMap, ffi::OsString, path::PathBuf, time::Duration};

/// The complete, explicit environment supplied to a child process.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProcessEnvironment {
    values: BTreeMap<OsString, OsString>,
}

impl ProcessEnvironment {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, name: impl Into<OsString>, value: impl Into<OsString>) {
        self.values.insert(name.into(), value.into());
    }

    pub fn get(&self, name: &str) -> Option<&std::ffi::OsStr> {
        self.values
            .get(std::ffi::OsStr::new(name))
            .map(OsString::as_os_str)
    }

    pub(super) fn values(&self) -> &BTreeMap<OsString, OsString> {
        &self.values
    }
}

/// An owned description of one hermetic process invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessCommand {
    program: PathBuf,
    arguments: Vec<OsString>,
    stdin: Vec<u8>,
    working_directory: PathBuf,
    environment: ProcessEnvironment,
    timeout: Duration,
    capture_limit: usize,
}

impl ProcessCommand {
    pub fn new(program: impl Into<PathBuf>, working_directory: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
            arguments: Vec::new(),
            stdin: Vec::new(),
            working_directory: working_directory.into(),
            environment: ProcessEnvironment::new(),
            timeout: DEFAULT_TIMEOUT,
            capture_limit: DEFAULT_PROCESS_CAPTURE_LIMIT,
        }
    }

    pub fn with_arguments(mut self, arguments: impl IntoIterator<Item = OsString>) -> Self {
        self.arguments = arguments.into_iter().collect();
        self
    }

    pub fn with_stdin(mut self, stdin: impl Into<Vec<u8>>) -> Self {
        self.stdin = stdin.into();
        self
    }

    pub fn with_environment(mut self, environment: ProcessEnvironment) -> Self {
        self.environment = environment;
        self
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn with_capture_limit(mut self, limit: usize) -> Self {
        self.capture_limit = limit;
        self
    }

    pub fn program(&self) -> &std::path::Path {
        &self.program
    }

    pub fn arguments(&self) -> &[OsString] {
        &self.arguments
    }

    pub fn stdin(&self) -> &[u8] {
        &self.stdin
    }

    pub fn working_directory(&self) -> &std::path::Path {
        &self.working_directory
    }

    pub fn environment(&self) -> &ProcessEnvironment {
        &self.environment
    }

    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    pub fn capture_limit(&self) -> usize {
        self.capture_limit
    }
}

/// A terminal process state, kept distinct from runner I/O failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessTermination {
    Code(i32),
    Signal(i32),
    TimedOut { limit: Duration },
}

/// One owned pipe whose operation could not complete normally.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessPipe {
    Stdin,
    Stdout,
    Stderr,
}

/// A process stream that produced more bytes than its retained capture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessCaptureOverflow {
    pipe: ProcessPipe,
    limit: usize,
    observed: usize,
}

impl ProcessCaptureOverflow {
    pub(super) fn new(pipe: ProcessPipe, limit: usize, observed: usize) -> Self {
        Self {
            pipe,
            limit,
            observed,
        }
    }

    pub fn pipe(&self) -> ProcessPipe {
        self.pipe
    }

    pub fn limit(&self) -> usize {
        self.limit
    }

    pub fn observed(&self) -> usize {
        self.observed
    }
}

/// A non-fatal pipe observation retained alongside process output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipeFailure {
    pipe: ProcessPipe,
    message: String,
}

impl PipeFailure {
    pub(super) fn new(pipe: ProcessPipe, message: impl Into<String>) -> Self {
        Self {
            pipe,
            message: message.into(),
        }
    }

    pub fn pipe(&self) -> ProcessPipe {
        self.pipe
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

/// Complete observations from one started child process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessObservation {
    pub(super) termination: ProcessTermination,
    pub(super) stdout: Vec<u8>,
    pub(super) stderr: Vec<u8>,
    pub(super) elapsed: Duration,
    pub(super) pipe_failures: Vec<PipeFailure>,
    pub(super) capture_overflows: Vec<ProcessCaptureOverflow>,
}

impl ProcessObservation {
    pub fn termination(&self) -> ProcessTermination {
        self.termination
    }

    pub fn stdout(&self) -> &[u8] {
        &self.stdout
    }

    pub fn stderr(&self) -> &[u8] {
        &self.stderr
    }

    pub fn elapsed(&self) -> Duration {
        self.elapsed
    }

    pub fn pipe_failures(&self) -> &[PipeFailure] {
        &self.pipe_failures
    }

    pub fn capture_overflows(&self) -> &[ProcessCaptureOverflow] {
        &self.capture_overflows
    }
}
