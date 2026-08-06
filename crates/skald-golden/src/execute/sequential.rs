use super::{
    remove_run_sandbox, BuildExecution, LeafExecution, LinkExecution, RuntimeExecution,
    SequentialExecution, SequentialOptions, StageStatus,
};
use crate::{
    compile::{compile_build, CompilationPurpose},
    execute_run, run_process, PlannedLeaf, PlannedLeafKind, ProcessCommand, ProcessTermination,
    SandboxRetention, SelectedPlan,
};
use skald_compiler::driver::{LinkObservation, ToolchainError};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

/// Executes selected dependencies in stable order without parallel scheduling.
pub fn execute_sequential(
    selected: &SelectedPlan<'_>,
    options: &SequentialOptions,
) -> SequentialExecution {
    let grouped = group_leaves(selected.leaves());
    let needs_runtime = selected
        .leaves()
        .iter()
        .any(|leaf| matches!(leaf.kind(), PlannedLeafKind::Run(_)));
    let runtime = needs_runtime.then(|| prepare_runtime(options));
    let runtime_passed = runtime
        .as_ref()
        .is_none_or(|runtime| runtime.status().passed());
    let mut builds = Vec::new();
    let mut leaves = Vec::new();

    for (build_id, selected_leaves) in grouped {
        let build = selected
            .plan()
            .build(build_id)
            .expect("selected leaf must reference a planned build");
        let first = selected_leaves[0];
        let purpose = match first.kind() {
            PlannedLeafKind::Run(_) => CompilationPurpose::Success,
            PlannedLeafKind::Compile(expectation) => CompilationPurpose::CompileFail(expectation),
        };
        let compilation = compile_build(build, purpose, options.compiler(), options.determinism());

        match first.kind() {
            PlannedLeafKind::Compile(_) => {
                let status = compilation_status(&compilation);
                for leaf in selected_leaves {
                    leaves.push(LeafExecution::new(
                        leaf.id().to_owned(),
                        Vec::new(),
                        status.clone(),
                    ));
                }
                builds.push(BuildExecution::new(
                    build.id().to_owned(),
                    compilation,
                    None,
                    status,
                ));
            }
            PlannedLeafKind::Run(_) => {
                let (link, build_status) = if !compilation.passed() {
                    (
                        Some(LinkExecution::new(
                            build.artifact_directory().join("program"),
                            None,
                            StageStatus::Cancelled {
                                dependency: format!("{}::compile", build.id()),
                            },
                        )),
                        compilation_status(&compilation),
                    )
                } else if !runtime_passed {
                    let status = StageStatus::Cancelled {
                        dependency: "runtime".to_owned(),
                    };
                    (
                        Some(LinkExecution::new(
                            build.artifact_directory().join("program"),
                            None,
                            status.clone(),
                        )),
                        status,
                    )
                } else {
                    let link = link_build(build, &compilation, options);
                    let status = link.status().clone();
                    (Some(link), status)
                };

                let executable = link
                    .as_ref()
                    .expect("native builds always have a link result")
                    .executable()
                    .to_path_buf();
                for leaf in selected_leaves {
                    let leaf_execution = if build_status.passed() {
                        execute_native_leaf(leaf, &executable, options)
                    } else {
                        LeafExecution::new(
                            leaf.id().to_owned(),
                            Vec::new(),
                            StageStatus::Cancelled {
                                dependency: format!("{}::link", build.id()),
                            },
                        )
                    };
                    leaves.push(leaf_execution);
                }
                builds.push(BuildExecution::new(
                    build.id().to_owned(),
                    compilation,
                    link,
                    build_status,
                ));
            }
        }
    }

    builds.sort_by(|left, right| left.build_id().cmp(right.build_id()));
    leaves.sort_by(|left, right| left.leaf_id().cmp(right.leaf_id()));
    SequentialExecution::new(runtime, builds, leaves)
}

fn group_leaves<'a>(leaves: &[&'a PlannedLeaf]) -> BTreeMap<&'a str, Vec<&'a PlannedLeaf>> {
    let mut grouped = BTreeMap::new();
    for leaf in leaves {
        grouped
            .entry(leaf.build_id())
            .or_insert_with(Vec::new)
            .push(*leaf);
    }
    grouped
}

fn prepare_runtime(options: &SequentialOptions) -> RuntimeExecution {
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

fn compilation_status(compilation: &crate::CompilationExecution) -> StageStatus {
    if compilation.passed() {
        StageStatus::Passed
    } else {
        StageStatus::Failed(format!(
            "compilation produced {} issue(s)",
            compilation.issues().len()
        ))
    }
}

fn link_build(
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

fn execute_native_leaf(
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
