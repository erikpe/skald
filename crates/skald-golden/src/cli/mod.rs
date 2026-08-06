//! Command-line parsing and user-facing read-only inspection.

mod options;

use crate::{build_plan, select};
use options::{Inspection, Options};
use std::{ffi::OsString, io::Write, process::ExitCode};

const PARTIAL_HELP: &str = "\
skald-golden - Skald golden-test runner\n\
\n\
Read-only operations available in this implementation:\n\
  --list                 List selected leaf IDs\n\
  --list-tests           List selected test and build IDs\n\
  --explain ID           Explain one fully resolved leaf\n\
  --filter GLOB          Include matching leaves; repeatable\n\
  --exclude GLOB         Exclude matching leaves; repeatable\n\
  --exact ID             Select one exact leaf\n\
  --variant NAME         Restrict variants; repeatable\n\
  --compiler-arg ARG     Append a compiler argument; repeatable\n\
  --allow-empty          Permit an empty selection\n";

/// Runs the command-line entry point.
///
/// Discovery and inspection are available without side effects. Test-process
/// execution remains unavailable until its later implementation stages.
pub fn run_cli(arguments: impl IntoIterator<Item = OsString>) -> ExitCode {
    let stdout = std::io::stdout();
    let stderr = std::io::stderr();
    run_cli_with_context(
        arguments,
        std::path::Path::new("tests/golden"),
        std::path::Path::new("build/golden/cases"),
        &mut stdout.lock(),
        &mut stderr.lock(),
    )
    .into()
}

fn run_cli_with_context(
    arguments: impl IntoIterator<Item = OsString>,
    golden_root: &std::path::Path,
    artifact_root: &std::path::Path,
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
    let Some(inspection) = options.inspection else {
        return usage_error(
            stderr,
            "test execution is not implemented yet; choose --list, --list-tests, or --explain",
        );
    };

    let plan = match build_plan(golden_root, artifact_root, &options.compiler_args) {
        Ok(plan) => plan,
        Err(error) => return usage_error(stderr, &error.to_string()),
    };
    let selection_options = match &inspection {
        Inspection::Explain(id) => options.selection.exact(id.clone()),
        Inspection::List | Inspection::ListTests => options.selection,
    };
    let selected = match select(&plan, &selection_options) {
        Ok(selected) => selected,
        Err(error) => return usage_error(stderr, &error.to_string()),
    };
    let output = match inspection {
        Inspection::List => selected.list(),
        Inspection::ListTests => selected.list_tests(),
        Inspection::Explain(id) => match selected.explain(&id) {
            Ok(explanation) => explanation,
            Err(error) => return usage_error(stderr, &error.to_string()),
        },
    };
    write_output(stdout, &output)
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
