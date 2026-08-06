use super::{
    PipeFailure, ProcessCommand, ProcessError, ProcessObservation, ProcessPipe, ProcessTermination,
};
use std::{
    io::{self, Read, Write},
    process::{Child, Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};

#[cfg(unix)]
use std::os::unix::process::{CommandExt, ExitStatusExt};

/// Runs one child with concurrent pipe handling and a per-process timeout.
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
    let mut child = command
        .spawn()
        .map_err(|source| ProcessError::new(request.program().to_path_buf(), "start", source))?;
    let stdin = child
        .stdin
        .take()
        .expect("configured child stdin must exist");
    let stdout = child
        .stdout
        .take()
        .expect("configured child stdout must exist");
    let stderr = child
        .stderr
        .take()
        .expect("configured child stderr must exist");
    let input = request.stdin().to_vec();

    let stdin_thread = thread::spawn(move || write_stdin(stdin, &input));
    let stdout_thread = thread::spawn(move || read_pipe(stdout));
    let stderr_thread = thread::spawn(move || read_pipe(stderr));

    let (status, timed_out) = wait_until(&mut child, request.timeout())
        .map_err(|source| ProcessError::new(request.program().to_path_buf(), "wait for", source))?;
    if timed_out {
        terminate_process_group(&mut child);
    }
    let status = match status {
        Some(status) => status,
        None => child
            .wait()
            .map_err(|source| ProcessError::new(request.program().to_path_buf(), "reap", source))?,
    };

    let mut failures = Vec::new();
    collect_write_result(stdin_thread.join(), &mut failures);
    let stdout = collect_read_result(stdout_thread.join(), ProcessPipe::Stdout, &mut failures);
    let stderr = collect_read_result(stderr_thread.join(), ProcessPipe::Stderr, &mut failures);
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
    })
}

fn write_stdin(mut stdin: std::process::ChildStdin, input: &[u8]) -> io::Result<()> {
    stdin.write_all(input)
}

fn read_pipe(mut pipe: impl Read) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    pipe.read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn wait_until(child: &mut Child, timeout: Duration) -> io::Result<(Option<ExitStatus>, bool)> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok((Some(status), false));
        }
        let now = Instant::now();
        if now >= deadline {
            return Ok((None, true));
        }
        thread::sleep((deadline - now).min(Duration::from_millis(5)));
    }
}

#[cfg(target_os = "linux")]
fn terminate_process_group(child: &mut Child) {
    use nix::{
        sys::signal::{killpg, Signal},
        unistd::Pid,
    };

    let _ = killpg(Pid::from_raw(child.id() as i32), Signal::SIGKILL);
    let _ = child.kill();
}

#[cfg(not(target_os = "linux"))]
fn terminate_process_group(child: &mut Child) {
    let _ = child.kill();
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
    result: thread::Result<io::Result<Vec<u8>>>,
    pipe: ProcessPipe,
    failures: &mut Vec<PipeFailure>,
) -> Vec<u8> {
    match result {
        Ok(Ok(bytes)) => bytes,
        Ok(Err(error)) => {
            failures.push(PipeFailure::new(pipe, error.to_string()));
            Vec::new()
        }
        Err(_) => {
            failures.push(PipeFailure::new(pipe, "output worker panicked"));
            Vec::new()
        }
    }
}
