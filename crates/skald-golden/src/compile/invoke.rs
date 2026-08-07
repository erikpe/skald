use super::{
    CompilationExecution, CompilationIssue, CompilationKind, CompilerConfig, CompilerObservation,
    Determinism,
};
use crate::{
    compare_stream, run_process, PlannedBuild, ProcessCommand, ProcessTermination,
    ResolvedCompileExpectation,
};
use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

pub(crate) enum CompilationPurpose<'a> {
    Success,
    CompileFail(&'a ResolvedCompileExpectation),
}

impl CompilationPurpose<'_> {
    fn kind(&self) -> CompilationKind {
        match self {
            Self::Success => CompilationKind::Success,
            Self::CompileFail(_) => CompilationKind::CompileFail,
        }
    }

    fn expected_status(&self) -> i32 {
        match self {
            Self::Success => 0,
            Self::CompileFail(_) => 1,
        }
    }
}

pub(crate) fn compile_build(
    build: &PlannedBuild,
    purpose: CompilationPurpose<'_>,
    config: &CompilerConfig,
    determinism: Determinism,
) -> CompilationExecution {
    let mut observations = Vec::new();
    let mut issues = Vec::new();
    if let Err(error) = fs::create_dir_all(build.artifact_directory()) {
        issues.push(CompilationIssue::Process(format!(
            "could not create artifact directory {}: {error}",
            build.artifact_directory().display()
        )));
        return CompilationExecution::new(
            build.id().to_owned(),
            purpose.kind(),
            observations,
            None,
            issues,
        );
    }

    for repetition in 0..determinism.compile_repetitions() {
        let assembly_path = assembly_path(build.artifact_directory(), repetition);
        if let Err(error) = remove_stale(&assembly_path) {
            issues.push(CompilationIssue::Process(error));
            break;
        }
        let timeout = build
            .timeout_seconds()
            .map(Duration::from_secs)
            .unwrap_or_else(|| config.default_timeout());
        let mut arguments = build.compiler_args().to_vec();
        arguments.extend([
            OsString::from("--emit"),
            OsString::from("asm"),
            OsString::from("-o"),
            assembly_path.as_os_str().to_owned(),
        ]);
        let working_directory = build
            .compiler_working_directory()
            .unwrap_or_else(|| config.working_directory());
        let command = ProcessCommand::new(config.executable(), working_directory)
            .with_arguments(arguments)
            .with_environment(config.environment().clone())
            .with_timeout(timeout);
        let mut process = match run_process(&command) {
            Ok(process) => process,
            Err(error) => {
                issues.push(CompilationIssue::Process(error.to_string()));
                observations.push(CompilerObservation::new(command, None, assembly_path, None));
                break;
            }
        };
        if let CompilationPurpose::CompileFail(expectation) = &purpose {
            if let Some(prefix) = expectation.stderr_prefix_to_strip() {
                process.strip_stderr_prefix(prefix);
            }
        }
        check_process(&process, &purpose, &mut issues);
        let assembly = if matches!(purpose, CompilationPurpose::Success)
            && process.termination() == ProcessTermination::Code(0)
        {
            match fs::read(&assembly_path) {
                Ok(assembly) => {
                    if std::str::from_utf8(&assembly).is_err() {
                        issues.push(CompilationIssue::NonUtf8Assembly(assembly_path.clone()));
                    }
                    Some(assembly)
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    issues.push(CompilationIssue::MissingAssembly(assembly_path.clone()));
                    None
                }
                Err(error) => {
                    issues.push(CompilationIssue::AssemblyRead {
                        path: assembly_path.clone(),
                        message: error.to_string(),
                    });
                    None
                }
            }
        } else {
            None
        };
        observations.push(CompilerObservation::new(
            command,
            Some(process),
            assembly_path,
            assembly,
        ));
    }

    if observations.len() == 2 {
        check_determinism(&observations, &purpose, &mut issues);
    }
    let mut stderr_comparison = None;
    if let CompilationPurpose::CompileFail(expectation) = &purpose {
        if let Some(process) = observations.first().and_then(CompilerObservation::process) {
            match compare_stream(expectation.stderr(), process.stderr()) {
                Ok(comparison) => {
                    if let Err(mismatch) = &comparison {
                        issues.push(CompilationIssue::StderrExpectation(mismatch.clone()));
                    }
                    stderr_comparison = Some(comparison);
                }
                Err(error) => issues.push(CompilationIssue::ExpectationLoad(error.to_string())),
            }
        }
    }
    CompilationExecution::new(
        build.id().to_owned(),
        purpose.kind(),
        observations,
        stderr_comparison,
        issues,
    )
}

fn check_process(
    process: &crate::ProcessObservation,
    purpose: &CompilationPurpose<'_>,
    issues: &mut Vec<CompilationIssue>,
) {
    if process.termination() != ProcessTermination::Code(purpose.expected_status()) {
        issues.push(CompilationIssue::Termination {
            expected: purpose.expected_status(),
            actual: process.termination(),
        });
    }
    issues.extend(
        process
            .pipe_failures()
            .iter()
            .cloned()
            .map(CompilationIssue::Pipe),
    );
    if !process.stdout().is_empty() {
        issues.push(CompilationIssue::UnexpectedStdout(
            process.stdout().to_vec(),
        ));
    }
    if matches!(purpose, CompilationPurpose::Success) && !process.stderr().is_empty() {
        issues.push(CompilationIssue::UnexpectedStderr(
            process.stderr().to_vec(),
        ));
    }
}

fn check_determinism(
    observations: &[CompilerObservation],
    purpose: &CompilationPurpose<'_>,
    issues: &mut Vec<CompilationIssue>,
) {
    let Some(first) = observations[0].process() else {
        return;
    };
    let Some(second) = observations[1].process() else {
        return;
    };
    match purpose {
        CompilationPurpose::Success => {
            if observations[0].assembly() != observations[1].assembly() {
                issues.push(CompilationIssue::NondeterministicAssembly);
            }
        }
        CompilationPurpose::CompileFail(_) => {
            if first.termination() != second.termination()
                || first.stdout() != second.stdout()
                || first.stderr() != second.stderr()
                || first.pipe_failures() != second.pipe_failures()
            {
                issues.push(CompilationIssue::NondeterministicDiagnostics);
            }
        }
    }
}

fn assembly_path(directory: &Path, repetition: usize) -> PathBuf {
    directory.join(if repetition == 0 {
        "assembly.s"
    } else {
        "assembly.repeat.s"
    })
}

fn remove_stale(path: &Path) -> Result<(), String> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "could not remove stale compiler artifact {}: {error}",
            path.display()
        )),
    }
}
