//! Concurrent pipe handling for one host toolchain process.

use std::{
    io::{self, Read, Write},
    process::{Child, Command, ExitStatus, Stdio},
    thread,
};

use super::{LinkInvocation, LinkObservation, ToolchainError};

pub(super) fn execute_link(invocation: &LinkInvocation) -> Result<LinkObservation, ToolchainError> {
    let mut child = Command::new(invocation.program())
        .args(invocation.arguments())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|source| ToolchainError::Start {
            tool: invocation.program().to_owned(),
            source,
        })?;

    let stdin = child
        .stdin
        .take()
        .expect("piped toolchain stdin must be available");
    let stdout = child
        .stdout
        .take()
        .expect("piped toolchain stdout must be available");
    let stderr = child
        .stderr
        .take()
        .expect("piped toolchain stderr must be available");

    thread::scope(|scope| {
        let stdin_worker = match thread::Builder::new()
            .name("skald-link-stdin".to_owned())
            .spawn_scoped(scope, || write_all(stdin, invocation.stdin()))
        {
            Ok(worker) => worker,
            Err(source) => {
                terminate_and_reap(&mut child);
                return Err(worker_start_error(invocation, "stdin", source));
            }
        };
        let stdout_worker = match thread::Builder::new()
            .name("skald-link-stdout".to_owned())
            .spawn_scoped(scope, || read_all(stdout))
        {
            Ok(worker) => worker,
            Err(source) => {
                terminate_and_reap(&mut child);
                return Err(worker_start_error(invocation, "stdout", source));
            }
        };
        let stderr_worker = match thread::Builder::new()
            .name("skald-link-stderr".to_owned())
            .spawn_scoped(scope, || read_all(stderr))
        {
            Ok(worker) => worker,
            Err(source) => {
                terminate_and_reap(&mut child);
                return Err(worker_start_error(invocation, "stderr", source));
            }
        };

        let status = wait_with_cleanup(&mut child);
        let stdin_result = join_worker(stdin_worker, "stdin", invocation)?;
        let stdout = join_worker(stdout_worker, "stdout", invocation)?
            .map_err(|source| pipe_error(invocation, "read stdout", source))?;
        let stderr = join_worker(stderr_worker, "stderr", invocation)?
            .map_err(|source| pipe_error(invocation, "read stderr", source))?;
        let status = status.map_err(|source| ToolchainError::Wait {
            tool: invocation.program().to_owned(),
            source,
        })?;

        // A rejecting tool may close stdin before consuming all assembly. The
        // failed status and EPIPE describe the same invocation, and scheduling
        // alone determines whether the writer observes the pipe closure.
        // Preserve the stable process failure and its captured output.
        if let Err(source) = stdin_result {
            if status.success() || source.kind() != io::ErrorKind::BrokenPipe {
                return Err(ToolchainError::WriteAssembly {
                    tool: invocation.program().to_owned(),
                    source,
                });
            }
        }

        Ok(LinkObservation::new(status.code(), stdout, stderr))
    })
}

fn write_all(mut stdin: impl Write, input: &[u8]) -> io::Result<()> {
    stdin.write_all(input)
}

fn read_all(mut output: impl Read) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    output.read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// Ensures a wait failure cannot leave a child running while pipe workers are
/// joined. A successful wait has already reaped the child and closed its pipe
/// endpoints before worker collection begins.
fn wait_with_cleanup(child: &mut Child) -> io::Result<ExitStatus> {
    match child.wait() {
        Ok(status) => Ok(status),
        Err(error) => {
            terminate_and_reap(child);
            Err(error)
        }
    }
}

fn terminate_and_reap(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn join_worker<T>(
    worker: thread::ScopedJoinHandle<'_, T>,
    pipe: &'static str,
    invocation: &LinkInvocation,
) -> Result<T, ToolchainError> {
    worker.join().map_err(|_| ToolchainError::Execute {
        tool: invocation.program().to_owned(),
        details: format!("{pipe} worker panicked"),
    })
}

fn pipe_error(
    invocation: &LinkInvocation,
    operation: &'static str,
    source: io::Error,
) -> ToolchainError {
    ToolchainError::Execute {
        tool: invocation.program().to_owned(),
        details: format!("could not {operation}: {source}"),
    }
}

fn worker_start_error(
    invocation: &LinkInvocation,
    pipe: &'static str,
    source: io::Error,
) -> ToolchainError {
    ToolchainError::Execute {
        tool: invocation.program().to_owned(),
        details: format!("could not start {pipe} worker: {source}"),
    }
}
