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
}

impl ProcessCommand {
    pub fn new(program: impl Into<PathBuf>, working_directory: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
            arguments: Vec::new(),
            stdin: Vec::new(),
            working_directory: working_directory.into(),
            environment: ProcessEnvironment::new(),
            timeout: Duration::from_secs(10),
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
}

impl ProcessObservation {
    pub(crate) fn strip_stderr_prefix(&mut self, prefix: &[u8]) {
        self.stderr = replace_bytes(&self.stderr, prefix, b"");
    }

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
}

fn replace_bytes(input: &[u8], needle: &[u8], replacement: &[u8]) -> Vec<u8> {
    if needle.is_empty() {
        return input.to_owned();
    }
    let mut output = Vec::with_capacity(input.len());
    let mut remaining = input;
    while let Some(index) = remaining
        .windows(needle.len())
        .position(|window| window == needle)
    {
        output.extend_from_slice(&remaining[..index]);
        output.extend_from_slice(replacement);
        remaining = &remaining[index + needle.len()..];
    }
    output.extend_from_slice(remaining);
    output
}

#[cfg(test)]
mod tests {
    use super::replace_bytes;

    #[test]
    fn byte_replacement_removes_every_non_overlapping_prefix() {
        assert_eq!(replace_bytes(b"/case/a\n/case/b", b"/case/", b""), b"a\nb");
        assert_eq!(replace_bytes(b"unchanged", b"", b"x"), b"unchanged");
    }
}
