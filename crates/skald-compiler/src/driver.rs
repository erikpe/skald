//! Pipeline orchestration and the implementation-independent CLI contract.
//!
//! This module may compose phases, but phase implementations must not depend
//! on the driver.

use std::{
    ffi::OsString,
    io::{self, Write},
};

const HELP: &str = "skac - the Skald compiler\n\nUsage: skac <input.ska> [-o <output>] [--emit asm]\n\nThe first compiler slice is not implemented yet.";
const EXIT_USAGE: i32 = 2;
const EXIT_IO_ERROR: i32 = 74;

/// Runs the current command-line scaffold and returns a process exit code.
pub fn run_cli<I>(args: I) -> i32
where
    I: IntoIterator<Item = OsString>,
{
    let stdout = io::stdout();
    let stderr = io::stderr();
    let mut stdout = stdout.lock();
    let mut stderr = stderr.lock();

    match run_cli_with_writers(args, &mut stdout, &mut stderr) {
        Ok(exit_code) => exit_code,
        Err(error) => {
            eprintln!("skac: failed to write command output: {error}");
            EXIT_IO_ERROR
        }
    }
}

fn run_cli_with_writers<I, Stdout, Stderr>(
    args: I,
    stdout: &mut Stdout,
    stderr: &mut Stderr,
) -> io::Result<i32>
where
    I: IntoIterator<Item = OsString>,
    Stdout: Write,
    Stderr: Write,
{
    let mut args = args.into_iter();
    let _program_name = args.next();

    match args.next().as_deref() {
        Some(arg) if arg == "--help" || arg == "-h" => {
            writeln!(stdout, "{HELP}")?;
            Ok(0)
        }
        Some(arg) if arg == "--version" => {
            writeln!(stdout, "skac {}", env!("CARGO_PKG_VERSION"))?;
            Ok(0)
        }
        Some(_) => {
            writeln!(
                stderr,
                "skac: the first vertical compiler slice is not implemented yet"
            )?;
            Ok(EXIT_USAGE)
        }
        None => {
            writeln!(stderr, "{HELP}")?;
            Ok(EXIT_USAGE)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(args: &[&str]) -> (i32, String, String) {
        let args = args.iter().map(OsString::from);
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let exit_code = run_cli_with_writers(args, &mut stdout, &mut stderr).unwrap();

        (
            exit_code,
            String::from_utf8(stdout).unwrap(),
            String::from_utf8(stderr).unwrap(),
        )
    }

    #[test]
    fn help_is_available_before_the_compiler_pipeline() {
        let (exit_code, stdout, stderr) = run(&["skac", "--help"]);

        assert_eq!(exit_code, 0);
        assert_eq!(stdout, format!("{HELP}\n"));
        assert!(stderr.is_empty());
    }

    #[test]
    fn version_is_available_before_the_compiler_pipeline() {
        let (exit_code, stdout, stderr) = run(&["skac", "--version"]);

        assert_eq!(exit_code, 0);
        assert_eq!(stdout, format!("skac {}\n", env!("CARGO_PKG_VERSION")));
        assert!(stderr.is_empty());
    }

    #[test]
    fn missing_input_is_a_usage_error() {
        let (exit_code, stdout, stderr) = run(&["skac"]);

        assert_eq!(exit_code, EXIT_USAGE);
        assert!(stdout.is_empty());
        assert_eq!(stderr, format!("{HELP}\n"));
    }

    #[test]
    fn source_compilation_reports_the_unimplemented_slice() {
        let (exit_code, stdout, stderr) = run(&["skac", "input.ska"]);

        assert_eq!(exit_code, EXIT_USAGE);
        assert!(stdout.is_empty());
        assert_eq!(
            stderr,
            "skac: the first vertical compiler slice is not implemented yet\n"
        );
    }
}
