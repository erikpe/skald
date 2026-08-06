use super::{
    remove_run_sandbox, LeafExecution, LinkExecution, RuntimeExecution, SchedulerOptions,
    SequentialExecution, SequentialOptions, StageStatus,
};
use crate::{
    execute_run, run_process, PlannedLeaf, PlannedLeafKind, ProcessCommand, ProcessTermination,
    SandboxRetention, SelectedPlan,
};
use skald_compiler::driver::{LinkObservation, ToolchainError};
use std::{
    fs,
    num::NonZeroUsize,
    path::{Path, PathBuf},
};

/// Executes selected dependencies in stable order without parallel scheduling.
pub fn execute_sequential(
    selected: &SelectedPlan<'_>,
    options: &SequentialOptions,
) -> SequentialExecution {
    super::scheduler::execute_parallel(selected, options, SchedulerOptions::new(NonZeroUsize::MIN))
}

pub(crate) fn prepare_runtime(options: &SequentialOptions) -> RuntimeExecution {
    let preparation = options.runtime();
    match run_process(preparation.command()) {
        Ok(process) => {
            let status = if process.termination() != ProcessTermination::Code(0) {
                StageStatus::Failed(format!(
                    "runtime preparation terminated with {:?}",
                    process.termination()
                ))
            } else if !process.pipe_failures().is_empty() {
                StageStatus::Failed("runtime preparation had a pipe failure".to_owned())
            } else if !preparation.archive().is_file() {
                StageStatus::Failed(format!(
                    "runtime preparation did not produce {}",
                    preparation.archive().display()
                ))
            } else {
                StageStatus::Passed
            };
            RuntimeExecution::new(
                preparation.command().clone(),
                preparation.archive().to_path_buf(),
                Some(process),
                status,
            )
        }
        Err(error) => RuntimeExecution::new(
            preparation.command().clone(),
            preparation.archive().to_path_buf(),
            None,
            StageStatus::Failed(error.to_string()),
        ),
    }
}

pub(crate) fn compilation_status(compilation: &crate::CompilationExecution) -> StageStatus {
    if compilation.passed() {
        StageStatus::Passed
    } else {
        StageStatus::Failed(format!(
            "compilation produced {} issue(s)",
            compilation.issues().len()
        ))
    }
}

pub(crate) fn link_build(
    build: &crate::PlannedBuild,
    compilation: &crate::CompilationExecution,
    options: &SequentialOptions,
) -> LinkExecution {
    let executable = build.artifact_directory().join("program");
    if let Err(error) = remove_stale(&executable) {
        return LinkExecution::new(executable, None, StageStatus::Failed(error));
    }
    let Some(assembly) = compilation.first_assembly() else {
        return LinkExecution::new(
            executable,
            None,
            StageStatus::Failed("successful compilation has no assembly bytes".to_owned()),
        );
    };
    let Ok(assembly) = std::str::from_utf8(assembly) else {
        return LinkExecution::new(
            executable,
            None,
            StageStatus::Failed("compiler assembly is not UTF-8".to_owned()),
        );
    };
    let mut process = None;
    let result = options
        .toolchain()
        .link_assembly_with(assembly, &executable, |invocation| {
            let command = ProcessCommand::new(
                PathBuf::from(invocation.program()),
                options.compiler().working_directory(),
            )
            .with_arguments(invocation.arguments().iter().cloned())
            .with_stdin(invocation.stdin().to_vec())
            .with_environment(options.linker_environment().clone())
            .with_timeout(options.linker_timeout());
            let observed = run_process(&command).map_err(|error| ToolchainError::Execute {
                tool: invocation.program().to_owned(),
                details: error.to_string(),
            })?;
            process = Some(observed.clone());
            if !observed.pipe_failures().is_empty() {
                return Err(ToolchainError::Execute {
                    tool: invocation.program().to_owned(),
                    details: "linker pipe operation failed".to_owned(),
                });
            }
            match observed.termination() {
                ProcessTermination::Code(code) => Ok(LinkObservation::new(
                    Some(code),
                    observed.stdout().to_vec(),
                    observed.stderr().to_vec(),
                )),
                ProcessTermination::Signal(_) => Ok(LinkObservation::new(
                    None,
                    observed.stdout().to_vec(),
                    observed.stderr().to_vec(),
                )),
                ProcessTermination::TimedOut { limit } => Err(ToolchainError::Execute {
                    tool: invocation.program().to_owned(),
                    details: format!("timed out after {limit:?}"),
                }),
            }
        });
    let status = match result {
        Ok(()) => StageStatus::Passed,
        Err(error) => StageStatus::Failed(error.to_string()),
    };
    LinkExecution::new(executable, process, status)
}

pub(crate) fn execute_native_leaf(
    leaf: &PlannedLeaf,
    executable: &Path,
    options: &SequentialOptions,
) -> LeafExecution {
    let PlannedLeafKind::Run(run) = leaf.kind() else {
        unreachable!("native leaf execution requires a planned run");
    };
    let repetitions = options.determinism().run_repetitions();
    let keep_until_compared =
        repetitions == 2 && options.execution().retention() == SandboxRetention::Failures;
    let execution_options = if keep_until_compared {
        options
            .execution()
            .clone()
            .with_retention(SandboxRetention::All)
    } else {
        options.execution().clone()
    };
    let mut executions = Vec::new();
    let mut failure = None;
    for _ in 0..repetitions {
        match execute_run(executable, run, &execution_options) {
            Ok(execution) => executions.push(execution),
            Err(error) => {
                failure = Some(error.to_string());
                break;
            }
        }
    }
    if failure.is_none()
        && executions.len() == 2
        && !same_native_observation(&executions[0], &executions[1])
    {
        failure = Some("native observations were nondeterministic".to_owned());
    }
    if failure.is_none() && executions.iter().any(|execution| !execution.passed()) {
        failure = Some("native expectation mismatch".to_owned());
    }
    if failure.is_none() && keep_until_compared {
        for execution in &mut executions {
            if let Err(error) = remove_run_sandbox(execution) {
                failure = Some(error.to_string());
                break;
            }
        }
    }
    let status = failure.map_or(StageStatus::Passed, StageStatus::Failed);
    LeafExecution::new(leaf.id().to_owned(), executions, status)
}

fn same_native_observation(left: &crate::RunExecution, right: &crate::RunExecution) -> bool {
    left.observation().termination() == right.observation().termination()
        && left.observation().stdout() == right.observation().stdout()
        && left.observation().stderr() == right.observation().stderr()
        && left.observation().pipe_failures() == right.observation().pipe_failures()
        && left.output_files() == right.output_files()
}

fn remove_stale(path: &Path) -> Result<(), String> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "could not remove stale linked executable {}: {error}",
            path.display()
        )),
    }
}
