//! Command-line parsing, inspection, and bounded parallel execution.

mod options;

use crate::{
    allowlisted_environment, build_plan, execute_parallel, locate_compiler, select, CompilerConfig,
    ExecutionOptions, ProcessCommand, RuntimePreparation, SchedulerOptions, SequentialOptions,
};
use options::{Inspection, Options};
use skald_compiler::driver::{Toolchain, C_COMPILER_ENV, RUNTIME_ARCHIVE_ENV};
use std::{
    ffi::OsString,
    io::Write,
    path::{Path, PathBuf},
    process::ExitCode,
    time::Duration,
};

const PARTIAL_HELP: &str = "\
skald-golden - Skald golden-test runner\n\
\n\
Execution and read-only inspection:\n\
  --list                 List selected leaf IDs\n\
  --list-tests           List selected test and build IDs\n\
  --explain ID           Explain one fully resolved leaf\n\
  --filter GLOB          Include matching leaves; repeatable\n\
  --exclude GLOB         Exclude matching leaves; repeatable\n\
  --exact ID             Select one exact leaf\n\
  --variant NAME         Restrict variants; repeatable\n\
  --compiler PATH        Use this skac executable\n\
  --compiler-arg ARG     Append a compiler argument; repeatable\n\
  --determinism MODE     Use off (default), compile, or full\n\
  --jobs N               Bound active processes; defaults to host parallelism\n\
  --fail-fast            Stop starting unrelated work after a failure\n\
  --allow-empty          Permit an empty selection\n";

/// Runs the command-line entry point.
///
/// Discovery, inspection, and bounded dependency scheduling are available.
/// Complete reporting arrives later.
pub fn run_cli(arguments: impl IntoIterator<Item = OsString>) -> ExitCode {
    let stdout = std::io::stdout();
    let stderr = std::io::stderr();
    run_cli_with_context(
        arguments,
        std::path::Path::new("tests/golden"),
        std::path::Path::new("build/golden/cases"),
        std::path::Path::new("."),
        &mut stdout.lock(),
        &mut stderr.lock(),
    )
    .into()
}

fn run_cli_with_context(
    arguments: impl IntoIterator<Item = OsString>,
    golden_root: &std::path::Path,
    artifact_root: &std::path::Path,
    repository_root: &std::path::Path,
    stdout: &mut impl Write,
    stderr: &mut impl Write,
) -> u8 {
    let options = match Options::parse(arguments) {
        Ok(options) => options,
        Err(error) => return usage_error(stderr, &error),
    };
    if options.help {
        return write_output(stdout, PARTIAL_HELP);
    }
    let plan = match build_plan(golden_root, artifact_root, &options.compiler_args) {
        Ok(plan) => plan,
        Err(error) => return usage_error(stderr, &error.to_string()),
    };
    let selection_options = match &options.inspection {
        Some(Inspection::Explain(id)) => options.selection.clone().exact(id.clone()),
        Some(Inspection::List | Inspection::ListTests) | None => options.selection.clone(),
    };
    let selected = match select(&plan, &selection_options) {
        Ok(selected) => selected,
        Err(error) => return usage_error(stderr, &error.to_string()),
    };
    if let Some(inspection) = options.inspection {
        let output = match inspection {
            Inspection::List => selected.list(),
            Inspection::ListTests => selected.list_tests(),
            Inspection::Explain(id) => match selected.explain(&id) {
                Ok(explanation) => explanation,
                Err(error) => return usage_error(stderr, &error.to_string()),
            },
        };
        return write_output(stdout, &output);
    }
    if selected.leaves().is_empty() {
        return write_output(stdout, "golden: no selected leaves\n");
    }
    let compiler = match locate_compiler(options.compiler.as_deref()) {
        Ok(compiler) => compiler,
        Err(error) => return usage_error(stderr, &error.to_string()),
    };
    let execution_options = stage_options(compiler, repository_root, options.determinism);
    let scheduler_options = options
        .jobs
        .map(SchedulerOptions::new)
        .unwrap_or_default()
        .with_fail_fast(options.fail_fast);
    let execution = execute_parallel(&selected, &execution_options, scheduler_options);
    let mut output = String::new();
    for leaf in execution.leaves() {
        let label = if leaf.status().passed() {
            "PASS"
        } else {
            "FAIL"
        };
        output.push_str(label);
        output.push(' ');
        output.push_str(leaf.leaf_id());
        output.push('\n');
    }
    if write_output(stdout, &output) != 0 {
        return 1;
    }
    u8::from(!execution.passed())
}

fn stage_options(
    compiler: PathBuf,
    repository_root: &Path,
    determinism: crate::Determinism,
) -> SequentialOptions {
    let environment = allowlisted_environment();
    let runtime_archive = std::env::var_os(RUNTIME_ARCHIVE_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(|| repository_root.join("build/runtime/libskald_runtime.a"));
    let compiler_config = CompilerConfig::new(compiler, repository_root)
        .with_environment(environment.clone())
        .with_default_timeout(Duration::from_secs(10));
    let runtime = RuntimePreparation::new(
        ProcessCommand::new("make", repository_root)
            .with_arguments([OsString::from("runtime")])
            .with_environment(environment.clone())
            .with_timeout(Duration::from_secs(120)),
        &runtime_archive,
    );
    let c_compiler = std::env::var_os(C_COMPILER_ENV).unwrap_or_else(|| OsString::from("cc"));
    SequentialOptions::new(
        compiler_config,
        runtime,
        Toolchain::new(c_compiler, runtime_archive),
        ExecutionOptions::new(repository_root.join("build/golden/tmp"))
            .with_inherited_environment(environment),
    )
    .with_determinism(determinism)
}

fn usage_error(stderr: &mut impl Write, message: &str) -> u8 {
    let _ = writeln!(stderr, "skald-golden: {message}");
    2
}

fn write_output(stdout: &mut impl Write, output: &str) -> u8 {
    match stdout.write_all(output.as_bytes()) {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("skald-golden: could not write output: {error}");
            1
        }
    }
}

#[cfg(test)]
mod tests;
