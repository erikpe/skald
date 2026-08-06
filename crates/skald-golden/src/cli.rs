//! Command-line parsing and user-facing option ownership.

use std::{ffi::OsString, process::ExitCode};

/// Runs the command-line entry point.
///
/// Execution is deliberately unavailable until the later roadmap tasks have
/// established discovery and process behavior.
pub fn run_cli(_arguments: impl IntoIterator<Item = OsString>) -> ExitCode {
    eprintln!("skald-golden: execution is not implemented yet");
    ExitCode::from(2)
}
