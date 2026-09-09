use super::{
    capture::{read_pipe, PipeCapture},
    PipeFailure, ProcessCaptureOverflow, ProcessCommand, ProcessError, ProcessObservation,
    ProcessPipe, ProcessTermination,
};
use std::{
    io::{self, Write},
    process::{Child, Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};

#[cfg(unix)]
use std::os::unix::process::{CommandExt, ExitStatusExt};

/// Runs one child and completes all three pipe operations within one deadline.
pub fn run_process(request: &ProcessCommand) -> Result<ProcessObservation, ProcessError> {
    let mut command = Command::new(request.program());
    command
        .args(request.arguments())
        .current_dir(request.working_directory())
        .env_clear()
        .envs(request.environment().values())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(unix)]
    command.process_group(0);

    let started = Instant::now();
    let child = command
        .spawn()
        .map_err(|source| ProcessError::new(request.program().to_path_buf(), "start", source))?;
    let mut process = RunningProcess::new(child);
    let stdin = process
        .child
        .stdin
        .take()
        .expect("configured child stdin must exist");
    let stdout = process
        .child
        .stdout
        .take()
        .expect("configured child stdout must exist");
    let stderr = process
        .child
        .stderr
        .take()
        .expect("configured child stderr must exist");
    let input = request.stdin().to_vec();
    let capture_limit = request.capture_limit();

    process.workers = Some(PipeWorkers {
        stdin: thread::spawn(move || write_stdin(stdin, &input)),
        stdout: thread::spawn(move || read_pipe(stdout, capture_limit)),
        stderr: thread::spawn(move || read_pipe(stderr, capture_limit)),
    });

    let (status, timed_out) = match process.wait_until_complete(request.timeout()) {
        Ok(result) => result,
        Err(source) => return Err(process.into_error(request, "wait for", source)),
    };
    if timed_out {
        if let Err(source) = process.terminate() {
            return Err(process.into_error(request, "terminate", source));
        }
    }
    let status = match status {
        Some(status) => status,
        None => match process.child.wait() {
            Ok(status) => status,
            Err(source) => return Err(process.into_error(request, "reap", source)),
        },
    };

    let (stdout, stderr, capture_overflows, failures) =
        process.collect_workers(request.capture_limit());
    process.armed = false;
    let termination = if timed_out {
        ProcessTermination::TimedOut {
            limit: request.timeout(),
        }
    } else {
        termination(status)
    };
    Ok(ProcessObservation {
        termination,
        stdout,
        stderr,
        elapsed: started.elapsed(),
        pipe_failures: failures,
        capture_overflows,
    })
}

fn write_stdin(mut stdin: std::process::ChildStdin, input: &[u8]) -> io::Result<()> {
    stdin.write_all(input)
}

struct PipeWorkers {
    stdin: thread::JoinHandle<io::Result<()>>,
    stdout: thread::JoinHandle<io::Result<PipeCapture>>,
    stderr: thread::JoinHandle<io::Result<PipeCapture>>,
}

impl PipeWorkers {
    fn is_finished(&self) -> bool {
        self.stdin.is_finished() && self.stdout.is_finished() && self.stderr.is_finished()
    }
}

struct RunningProcess {
    child: Child,
    process_group: u32,
    workers: Option<PipeWorkers>,
    armed: bool,
}

impl RunningProcess {
    fn new(child: Child) -> Self {
        let process_group = child.id();
        Self {
            child,
            process_group,
            workers: None,
            armed: true,
        }
    }

    fn wait_until_complete(&mut self, timeout: Duration) -> io::Result<(Option<ExitStatus>, bool)> {
        let deadline = Instant::now() + timeout;
        let mut status = None;
        loop {
            if status.is_none() {
                status = self.child.try_wait()?;
            }
            let workers_finished = self
                .workers
                .as_ref()
                .map(PipeWorkers::is_finished)
                .unwrap_or(true);
            if status.is_some() && workers_finished {
                return Ok((status, false));
            }
            let now = Instant::now();
            if now >= deadline {
                return Ok((status, true));
            }
            thread::sleep((deadline - now).min(Duration::from_millis(5)));
        }
    }

    fn terminate(&mut self) -> io::Result<()> {
        terminate_process_group(self.process_group, &mut self.child)
    }

    fn collect_workers(
        &mut self,
        capture_limit: usize,
    ) -> (
        Vec<u8>,
        Vec<u8>,
        Vec<ProcessCaptureOverflow>,
        Vec<PipeFailure>,
    ) {
        let workers = self
            .workers
            .take()
            .expect("started process must own pipe workers");
        let mut failures = Vec::new();
        collect_write_result(workers.stdin.join(), &mut failures);
        let (stdout, stdout_overflow) = collect_read_result(
            workers.stdout.join(),
            ProcessPipe::Stdout,
            capture_limit,
            &mut failures,
        );
        let (stderr, stderr_overflow) = collect_read_result(
            workers.stderr.join(),
            ProcessPipe::Stderr,
            capture_limit,
            &mut failures,
        );
        let capture_overflows = stdout_overflow.into_iter().chain(stderr_overflow).collect();
        (stdout, stderr, capture_overflows, failures)
    }

    fn cleanup(&mut self) -> Vec<String> {
        let mut failures = Vec::new();
        if let Err(error) = self.terminate() {
            failures.push(format!("terminate process group: {error}"));
        }
        if let Err(error) = self.child.wait() {
            failures.push(format!("reap child: {error}"));
        }
        if self.workers.is_some() {
            let (_, _, _, pipe_failures) = self.collect_workers(0);
            failures.extend(pipe_failures.into_iter().map(|failure| {
                format!("complete {:?} pipe: {}", failure.pipe(), failure.message())
            }));
        }
        failures
    }

    fn into_error(
        mut self,
        request: &ProcessCommand,
        action: &'static str,
        source: io::Error,
    ) -> ProcessError {
        let cleanup_failures = self.cleanup();
        self.armed = false;
        ProcessError::new(request.program().to_path_buf(), action, source)
            .with_cleanup_failures(cleanup_failures)
    }
}

impl Drop for RunningProcess {
    fn drop(&mut self) {
        if self.armed {
            let _ = self.cleanup();
        }
    }
}

#[cfg(target_os = "linux")]
fn terminate_process_group(process_group: u32, child: &mut Child) -> io::Result<()> {
    use nix::{
        errno::Errno,
        sys::signal::{killpg, Signal},
        unistd::Pid,
    };

    match killpg(Pid::from_raw(process_group as i32), Signal::SIGKILL) {
        Ok(()) | Err(Errno::ESRCH) => Ok(()),
        Err(error) => child.kill().map_err(|fallback| {
            io::Error::other(format!(
                "could not kill process group ({error}) or direct child ({fallback})"
            ))
        }),
    }
}

#[cfg(not(target_os = "linux"))]
fn terminate_process_group(_process_group: u32, child: &mut Child) -> io::Result<()> {
    match child.kill() {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::InvalidInput => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(unix)]
fn termination(status: ExitStatus) -> ProcessTermination {
    match status.code() {
        Some(code) => ProcessTermination::Code(code),
        None => ProcessTermination::Signal(status.signal().unwrap_or_default()),
    }
}

#[cfg(not(unix))]
fn termination(status: ExitStatus) -> ProcessTermination {
    ProcessTermination::Code(status.code().unwrap_or(-1))
}

fn collect_write_result(result: thread::Result<io::Result<()>>, failures: &mut Vec<PipeFailure>) {
    match result {
        Ok(Ok(())) => {}
        Ok(Err(error)) if error.kind() == io::ErrorKind::BrokenPipe => {}
        Ok(Err(error)) => failures.push(PipeFailure::new(ProcessPipe::Stdin, error.to_string())),
        Err(_) => failures.push(PipeFailure::new(
            ProcessPipe::Stdin,
            "stdin worker panicked",
        )),
    }
}

fn collect_read_result(
    result: thread::Result<io::Result<PipeCapture>>,
    pipe: ProcessPipe,
    limit: usize,
    failures: &mut Vec<PipeFailure>,
) -> (Vec<u8>, Option<ProcessCaptureOverflow>) {
    match result {
        Ok(Ok(capture)) => {
            let (bytes, observed) = capture.into_parts();
            let overflow =
                (observed > limit).then(|| ProcessCaptureOverflow::new(pipe, limit, observed));
            (bytes, overflow)
        }
        Ok(Err(error)) => {
            failures.push(PipeFailure::new(pipe, error.to_string()));
            (Vec::new(), None)
        }
        Err(_) => {
            failures.push(PipeFailure::new(pipe, "output worker panicked"));
            (Vec::new(), None)
        }
    }
}
