//! Command-line parsing, inspection, and bounded parallel execution.

mod options;

use crate::process::DEFAULT_TIMEOUT;
use crate::{
    allowlisted_environment, build_plan, execute_parallel, locate_compiler, render_report, select,
    CompilerConfig, ExecutionOptions, PlannedLeafKind, ProcessCommand, Report, ReportFormat,
    ReportOptions, RuntimePreparation, SandboxRetention, SchedulerOptions, SequentialOptions,
};
use options::{Inspection, Options};
use skald_compiler::driver::{Toolchain, C_COMPILER_ENV, RUNTIME_ARCHIVE_ENV};
use std::{
    collections::BTreeSet,
    ffi::OsString,
    io::{self, Write},
    path::{Path, PathBuf},
    process::ExitCode,
    time::Duration,
};

const HELP: &str = r#"skald-golden - Skald golden-test runner

Execution and read-only inspection:
  --list                 List selected leaf IDs
  --list-tests           List selected test and build IDs
  --explain ID           Explain one fully resolved leaf
  --filter GLOB          Include matching leaves; repeatable
  --exclude GLOB         Exclude matching leaves; repeatable
  --exact ID             Select one exact leaf
  --variant NAME         Restrict variants; repeatable
  --compiler PATH        Use this skac executable
  --compiler-arg ARG     Append a compiler argument; repeatable
  --determinism MODE     Use off (default), compile, or full
  --jobs N               Bound active processes; defaults to host parallelism
  --timeout SECONDS      Override the 60-second timeout for each process
  --fail-fast            Stop starting unrelated work after a failure
  --show-output          Show captured output for passing cases too
  --slowest N            Report the N slowest completed leaf IDs
  --format FORMAT        Emit human (default), json, or junit output
  --keep-all-artifacts   Retain passing run sandboxes for debugging
  --allow-empty          Permit an empty selection
  --help, -h             Show this help
"#;

/// Runs the command-line entry point.
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
        return write_output(stdout, stderr, HELP).status();
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
        return write_output(stdout, stderr, &output).status();
    }
    let report_options = ReportOptions::default()
        .with_show_output(options.show_output)
        .with_slowest(options.slowest);
    if selected.leaves().is_empty() {
        let report = Report::empty(&selected, options.determinism, report_options);
        let output = match render_report(&report, options.format) {
            Ok(output) => output,
            Err(error) => return usage_error(stderr, &error),
        };
        return write_output(stdout, stderr, &output).status();
    }
    let compiler = match locate_compiler(options.compiler.as_deref()) {
        Ok(compiler) => compiler,
        Err(error) => return usage_error(stderr, &error.to_string()),
    };
    if options.format == ReportFormat::Human {
        match write_output(
            stdout,
            stderr,
            &selection_header(&selected, options.determinism),
        ) {
            Output::Written => {}
            Output::BrokenPipe => return 0,
            Output::Failed => return 1,
        }
    }
    let retention = if options.keep_all_artifacts {
        SandboxRetention::All
    } else {
        SandboxRetention::Failures
    };
    let execution_options = stage_options(
        compiler,
        repository_root,
        options.determinism,
        options.timeout,
        retention,
    );
    let scheduler_options = options
        .jobs
        .map(SchedulerOptions::new)
        .unwrap_or_default()
        .with_fail_fast(options.fail_fast);
    let execution = execute_parallel(&selected, &execution_options, scheduler_options);
    let report = Report::new(&selected, &execution, options.determinism, report_options);
    let output = match render_report(&report, options.format) {
        Ok(output) => output,
        Err(error) => return usage_error(stderr, &error),
    };
    match write_output(stdout, stderr, &output) {
        Output::Written => u8::from(!report.passed()),
        Output::BrokenPipe => 0,
        Output::Failed => 1,
    }
}

fn stage_options(
    compiler: PathBuf,
    repository_root: &Path,
    determinism: crate::Determinism,
    timeout: Option<Duration>,
    retention: SandboxRetention,
) -> SequentialOptions {
    let environment = allowlisted_environment();
    let runtime_archive = std::env::var_os(RUNTIME_ARCHIVE_ENV)
        .map(PathBuf::from)
        .unwrap_or_else(|| repository_root.join("build/runtime/libskald_runtime.a"));
    let default_timeout = timeout.unwrap_or(DEFAULT_TIMEOUT);
    let compiler_config = CompilerConfig::new(compiler, repository_root)
        .with_environment(environment.clone())
        .with_default_timeout(default_timeout);
    let runtime = RuntimePreparation::new(
        ProcessCommand::new("make", repository_root)
            .with_arguments([OsString::from("runtime")])
            .with_environment(environment.clone())
            .with_timeout(timeout.unwrap_or(Duration::from_secs(120))),
        &runtime_archive,
    );
    let c_compiler = std::env::var_os(C_COMPILER_ENV).unwrap_or_else(|| OsString::from("cc"));
    SequentialOptions::new(
        compiler_config,
        runtime,
        Toolchain::new(c_compiler, runtime_archive),
        ExecutionOptions::new(repository_root.join("build/golden/tmp"))
            .with_inherited_environment(environment)
            .with_default_timeout(default_timeout)
            .with_retention(retention),
    )
    .with_determinism(determinism)
    .with_linker_timeout(default_timeout)
}

fn usage_error(stderr: &mut impl Write, message: &str) -> u8 {
    let _ = writeln!(stderr, "skald-golden: {message}");
    2
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Output {
    Written,
    BrokenPipe,
    Failed,
}

impl Output {
    fn status(self) -> u8 {
        match self {
            Self::Written | Self::BrokenPipe => 0,
            Self::Failed => 1,
        }
    }
}

fn write_output(stdout: &mut impl Write, stderr: &mut impl Write, output: &str) -> Output {
    match stdout
        .write_all(output.as_bytes())
        .and_then(|()| stdout.flush())
    {
        Ok(()) => Output::Written,
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Output::BrokenPipe,
        Err(error) => {
            let _ = writeln!(stderr, "skald-golden: could not write output: {error}");
            Output::Failed
        }
    }
}

fn selection_header(selected: &crate::SelectedPlan<'_>, determinism: crate::Determinism) -> String {
    let counts = Report::selection_counts(selected);
    let mode = match determinism {
        crate::Determinism::Off => "off",
        crate::Determinism::Compile => "compile",
        crate::Determinism::Full => "full",
    };
    let builds = selected
        .leaves()
        .iter()
        .map(|leaf| leaf.build_id())
        .collect::<BTreeSet<_>>()
        .len();
    let (compile_fail, runs) =
        selected
            .leaves()
            .iter()
            .fold((0usize, 0usize), |(compile_fail, runs), leaf| {
                match leaf.kind() {
                    PlannedLeafKind::Compile(_) => (compile_fail + 1, runs),
                    PlannedLeafKind::Run(_) => (compile_fail, runs + 1),
                }
            });
    format!(
        "golden: determinism {mode}; selected {} specs, {} source tests, {builds} builds, {compile_fail} compile-fail leaves, {runs} named runs\n",
        counts.specs, counts.source_tests,
    )
}

#[cfg(test)]
mod tests;
