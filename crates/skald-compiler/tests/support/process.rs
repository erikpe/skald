use std::{
    error::Error,
    ffi::{OsStr, OsString},
    fmt, io,
    process::{Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};

use super::temporary::TemporaryFile;

const POLL_INTERVAL: Duration = Duration::from_millis(10);
pub(crate) const DEFAULT_DIAGNOSTIC_OUTPUT_LIMIT: usize = 64 * 1024;

#[derive(Clone, Copy, Debug)]
pub(crate) struct TestProcessPolicy {
    timeout: Duration,
    diagnostic_output_limit: usize,
}

impl TestProcessPolicy {
    pub(crate) fn with_default_diagnostic_limit(timeout: Duration) -> Self {
        Self::new(timeout, DEFAULT_DIAGNOSTIC_OUTPUT_LIMIT)
    }

    pub(crate) fn new(timeout: Duration, diagnostic_output_limit: usize) -> Self {
        Self {
            timeout,
            diagnostic_output_limit,
        }
    }
}

#[derive(Debug)]
pub(crate) struct CurrentTestProcessRequest {
    test_name: String,
    environment: Vec<(OsString, OsString)>,
    policy: TestProcessPolicy,
}

impl CurrentTestProcessRequest {
    pub(crate) fn new(test_name: impl Into<String>, policy: TestProcessPolicy) -> Self {
        Self {
            test_name: test_name.into(),
            environment: Vec::new(),
            policy,
        }
    }

    pub(crate) fn with_environment(
        mut self,
        name: impl AsRef<OsStr>,
        value: impl AsRef<OsStr>,
    ) -> Self {
        self.environment
            .push((name.as_ref().to_owned(), value.as_ref().to_owned()));
        self
    }
}

#[derive(Debug)]
pub(crate) struct CurrentTestProcessObservation {
    pub(crate) termination: TestProcessTermination,
    pub(crate) stdout: BoundedOutput,
    pub(crate) stderr: BoundedOutput,
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum TestProcessTermination {
    Completed(ExitStatus),
    TimedOut,
}

#[derive(Debug)]
pub(crate) struct BoundedOutput {
    retained: Vec<u8>,
    observed_length: u64,
}

impl BoundedOutput {
    pub(crate) fn retained(&self) -> &[u8] {
        &self.retained
    }

    pub(crate) fn observed_length(&self) -> u64 {
        self.observed_length
    }

    pub(crate) fn overflowed(&self) -> bool {
        self.observed_length > self.retained.len() as u64
    }
}

#[derive(Debug)]
pub(crate) enum CurrentTestProcessError {
    LocateExecutable(io::Error),
    CreateStdoutCapture(io::Error),
    CreateStderrCapture(io::Error),
    OpenStdoutCapture(io::Error),
    OpenStderrCapture(io::Error),
    Spawn(io::Error),
    Wait(io::Error),
    Kill(io::Error),
    Reap(io::Error),
    ReadStdout(io::Error),
    ReadStderr(io::Error),
}

impl fmt::Display for CurrentTestProcessError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (operation, source) = match self {
            Self::LocateExecutable(source) => ("locate the current test executable", source),
            Self::CreateStdoutCapture(source) => ("create the stdout capture", source),
            Self::CreateStderrCapture(source) => ("create the stderr capture", source),
            Self::OpenStdoutCapture(source) => ("open the stdout capture", source),
            Self::OpenStderrCapture(source) => ("open the stderr capture", source),
            Self::Spawn(source) => ("spawn the current test executable", source),
            Self::Wait(source) => ("poll the current test executable", source),
            Self::Kill(source) => ("kill the timed-out test executable", source),
            Self::Reap(source) => ("reap the timed-out test executable", source),
            Self::ReadStdout(source) => ("read the test executable's stdout", source),
            Self::ReadStderr(source) => ("read the test executable's stderr", source),
        };
        write!(formatter, "failed to {operation}: {source}")
    }
}

impl Error for CurrentTestProcessError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(match self {
            Self::LocateExecutable(source)
            | Self::CreateStdoutCapture(source)
            | Self::CreateStderrCapture(source)
            | Self::OpenStdoutCapture(source)
            | Self::OpenStderrCapture(source)
            | Self::Spawn(source)
            | Self::Wait(source)
            | Self::Kill(source)
            | Self::Reap(source)
            | Self::ReadStdout(source)
            | Self::ReadStderr(source) => source,
        })
    }
}

pub(crate) fn run_current_test_process(
    request: CurrentTestProcessRequest,
) -> Result<CurrentTestProcessObservation, CurrentTestProcessError> {
    let executable = std::env::current_exe().map_err(CurrentTestProcessError::LocateExecutable)?;
    let stdout = TemporaryFile::new("test-process-stdout")
        .map_err(CurrentTestProcessError::CreateStdoutCapture)?;
    let stderr = TemporaryFile::new("test-process-stderr")
        .map_err(CurrentTestProcessError::CreateStderrCapture)?;
    let stdout_file = stdout
        .open_for_output()
        .map_err(CurrentTestProcessError::OpenStdoutCapture)?;
    let stderr_file = stderr
        .open_for_output()
        .map_err(CurrentTestProcessError::OpenStderrCapture)?;

    let mut child = Command::new(executable)
        .args(["--exact", &request.test_name, "--nocapture"])
        .envs(request.environment)
        .stdout(Stdio::from(stdout_file))
        .stderr(Stdio::from(stderr_file))
        .spawn()
        .map_err(CurrentTestProcessError::Spawn)?;

    let deadline = Instant::now() + request.policy.timeout;
    let termination = loop {
        match child.try_wait() {
            Ok(Some(status)) => break TestProcessTermination::Completed(status),
            Ok(None) => {}
            Err(source) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(CurrentTestProcessError::Wait(source));
            }
        }

        let now = Instant::now();
        if now >= deadline {
            if let Err(source) = child.kill() {
                if !matches!(child.try_wait(), Ok(Some(_))) {
                    return Err(CurrentTestProcessError::Kill(source));
                }
            } else {
                child.wait().map_err(CurrentTestProcessError::Reap)?;
            }
            break TestProcessTermination::TimedOut;
        }
        thread::sleep(POLL_INTERVAL.min(deadline.saturating_duration_since(now)));
    };

    let limit = request.policy.diagnostic_output_limit;
    let (retained, observed_length) = stdout
        .read_prefix(limit)
        .map_err(CurrentTestProcessError::ReadStdout)?;
    let stdout = BoundedOutput {
        retained,
        observed_length,
    };
    let (retained, observed_length) = stderr
        .read_prefix(limit)
        .map_err(CurrentTestProcessError::ReadStderr)?;
    let stderr = BoundedOutput {
        retained,
        observed_length,
    };

    Ok(CurrentTestProcessObservation {
        termination,
        stdout,
        stderr,
    })
}
