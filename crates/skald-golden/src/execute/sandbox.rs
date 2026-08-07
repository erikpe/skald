use super::{
    model::{
        ExecutionOptions, OutputFileMismatch, OutputFileObservation, RunExecution,
        RunExecutionParts, RunMismatch, SandboxRetention,
    },
    template::TemporaryPaths,
    ExecutionError,
};
use crate::{
    compare_exit, compare_stream, decode_arguments, load_bytes, run_process, MatcherOutcome,
    PlannedRun, ProcessCommand, ResolvedWorkingDirectory, StreamComparison,
};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

static NEXT_SANDBOX: AtomicU64 = AtomicU64::new(0);

struct ObservedRun {
    command: ProcessCommand,
    process: crate::ProcessObservation,
    stdout: StreamComparison,
    stderr: StreamComparison,
    output_files: Vec<OutputFileObservation>,
    mismatches: Vec<RunMismatch>,
}

/// Executes one resolved run in a private data sandbox.
pub fn execute_run(
    executable: &Path,
    run: &PlannedRun,
    options: &ExecutionOptions,
) -> Result<RunExecution, ExecutionError> {
    let sandbox = create_sandbox(options.temporary_root())?;
    match execute_in_sandbox(executable, run, options, &sandbox) {
        Ok(execution) => {
            let retained =
                options.retention() == SandboxRetention::All || !execution.mismatches.is_empty();
            if !retained {
                remove_sandbox(&sandbox)?;
            }
            Ok(RunExecution::from_parts(RunExecutionParts {
                command: execution.command,
                sandbox,
                retained,
                observation: execution.process,
                stdout_comparison: execution.stdout,
                stderr_comparison: execution.stderr,
                output_files: execution.output_files,
                mismatches: execution.mismatches,
            }))
        }
        Err(error) => Err(error.with_sandbox(sandbox)),
    }
}

pub(crate) fn remove_run_sandbox(execution: &mut RunExecution) -> Result<(), ExecutionError> {
    if execution.sandbox().exists() {
        remove_sandbox(execution.sandbox())?;
    }
    execution.mark_removed();
    Ok(())
}

fn execute_in_sandbox(
    executable: &Path,
    run: &PlannedRun,
    options: &ExecutionOptions,
    sandbox: &Path,
) -> Result<ObservedRun, ExecutionError> {
    let names = run
        .input_files()
        .iter()
        .map(|file| file.name().to_owned())
        .chain(
            run.expectation()
                .output_files()
                .iter()
                .map(|file| file.name().to_owned()),
        );
    let temporary_paths = TemporaryPaths::new(sandbox, names);
    for file in run.input_files() {
        let contents = load_bytes(file.contents()).map_err(|source| {
            ExecutionError::source("could not load temporary input file", source)
        })?;
        let path = temporary_paths.path(file.name());
        fs::write(path, contents).map_err(|source| {
            ExecutionError::io(
                path.to_path_buf(),
                "could not write temporary input file",
                source,
            )
        })?;
    }

    let arguments = decode_arguments(run.args())
        .map_err(|source| ExecutionError::source("could not load process arguments", source))?
        .iter()
        .map(|argument| temporary_paths.substitute_argument(argument))
        .collect::<Result<Vec<_>, _>>()?;
    let stdin =
        temporary_paths
            .substitute(&load_bytes(run.stdin()).map_err(|source| {
                ExecutionError::source("could not load process stdin", source)
            })?)?;
    let working_directory = match run.cwd() {
        ResolvedWorkingDirectory::Private => sandbox,
        ResolvedWorkingDirectory::Fixture(path) => path,
    };
    let mut environment = options.inherited_environment().clone();
    environment.insert("TMPDIR", sandbox.as_os_str());
    for (name, value) in run.env() {
        environment.insert(name, value);
    }
    let timeout = run
        .timeout_seconds()
        .map(Duration::from_secs)
        .unwrap_or_else(|| options.default_timeout());
    let request = ProcessCommand::new(executable, working_directory)
        .with_arguments(arguments)
        .with_stdin(stdin)
        .with_environment(environment)
        .with_timeout(timeout);
    let observation = run_process(&request)
        .map_err(|source| ExecutionError::source("could not execute run", source))?;
    let mut mismatches = observation
        .pipe_failures()
        .iter()
        .cloned()
        .map(RunMismatch::Pipe)
        .collect::<Vec<_>>();
    let expectation = run.expectation();
    if !compare_exit(expectation.exit(), observation.termination()) {
        mismatches.push(RunMismatch::Exit {
            expected: expectation.exit(),
            actual: observation.termination(),
        });
    }
    let stdout = compare_stream(expectation.stdout(), observation.stdout());
    collect_stream_mismatches(&stdout, &mut mismatches, true);
    let stderr = compare_stream(expectation.stderr(), observation.stderr());
    collect_stream_mismatches(&stderr, &mut mismatches, false);
    let output_files = compare_output_files(run, &temporary_paths, &mut mismatches)?;
    Ok(ObservedRun {
        command: request,
        process: observation,
        stdout,
        stderr,
        output_files,
        mismatches,
    })
}

fn collect_stream_mismatches(
    comparison: &StreamComparison,
    mismatches: &mut Vec<RunMismatch>,
    stdout: bool,
) {
    for outcome in comparison.outcomes() {
        match outcome {
            MatcherOutcome::Matched(_) => {}
            MatcherOutcome::Mismatched(mismatch) => mismatches.push(if stdout {
                RunMismatch::Stdout(mismatch.clone())
            } else {
                RunMismatch::Stderr(mismatch.clone())
            }),
            MatcherOutcome::LoadFailed(failure) => mismatches.push(if stdout {
                RunMismatch::StdoutLoad(failure.clone())
            } else {
                RunMismatch::StderrLoad(failure.clone())
            }),
        }
    }
}

fn compare_output_files(
    run: &PlannedRun,
    paths: &TemporaryPaths,
    mismatches: &mut Vec<RunMismatch>,
) -> Result<Vec<OutputFileObservation>, ExecutionError> {
    let mut observations = Vec::new();
    for file in run.expectation().output_files() {
        let expected = load_bytes(file.contents()).map_err(|source| {
            ExecutionError::source("could not load output-file expectation", source)
        })?;
        let path = paths.path(file.name());
        let actual = match fs::read(path) {
            Ok(contents) => Some(contents),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(source) => {
                return Err(ExecutionError::io(
                    path.to_path_buf(),
                    "could not read temporary output file",
                    source,
                ));
            }
        };
        if actual.as_deref() != Some(expected.as_slice()) {
            mismatches.push(RunMismatch::OutputFile(OutputFileMismatch::new(
                file.name().to_owned(),
                expected,
                actual.clone(),
            )));
        }
        observations.push(OutputFileObservation::new(file.name().to_owned(), actual));
    }
    Ok(observations)
}

fn create_sandbox(root: &Path) -> Result<PathBuf, ExecutionError> {
    fs::create_dir_all(root).map_err(|source| {
        ExecutionError::io(
            root.to_path_buf(),
            "could not create temporary root",
            source,
        )
    })?;
    for _ in 0..1000 {
        let sequence = NEXT_SANDBOX.fetch_add(1, Ordering::Relaxed);
        let path = root.join(format!("run-{}-{sequence}", std::process::id()));
        match fs::create_dir(&path) {
            Ok(()) => {
                make_private(&path)?;
                return fs::canonicalize(&path).map_err(|source| {
                    ExecutionError::io(path, "could not resolve private run directory", source)
                });
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(source) => {
                return Err(ExecutionError::io(
                    path,
                    "could not create private run directory",
                    source,
                ));
            }
        }
    }
    Err(ExecutionError::plain(
        "could not allocate a unique private run directory",
    ))
}

#[cfg(unix)]
fn make_private(path: &Path) -> Result<(), ExecutionError> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(|source| {
        ExecutionError::io(
            path.to_path_buf(),
            "could not make run directory private",
            source,
        )
    })
}

#[cfg(not(unix))]
fn make_private(_path: &Path) -> Result<(), ExecutionError> {
    Ok(())
}

fn remove_sandbox(path: &Path) -> Result<(), ExecutionError> {
    fs::remove_dir_all(path).map_err(|source| {
        ExecutionError::io(
            path.to_path_buf(),
            "could not remove passing run directory",
            source,
        )
    })
}
