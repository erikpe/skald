use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
};

use crate::{
    backend::Target,
    diagnostics::render_diagnostics,
    syntax::{EXCESSIVE_NESTING, MAX_SYNTAX_NESTING},
    test_support::{TemporaryDirectory, CANONICAL_ERROR_SOURCE, CANONICAL_STR_SOURCE},
};

use super::*;

fn run(args: &[&str]) -> (i32, String, String) {
    run_with_toolchain(args, &Toolchain::new("false", "missing-runtime.a"))
}

fn run_with_toolchain(args: &[&str], toolchain: &Toolchain) -> (i32, String, String) {
    let args = args.iter().map(OsString::from);
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let exit_code = run_cli_with_context(args, &mut stdout, &mut stderr, toolchain).unwrap();

    (
        exit_code,
        String::from_utf8(stdout).unwrap(),
        String::from_utf8(stderr).unwrap(),
    )
}

fn temporary_artifacts(directory: &Path) -> Vec<PathBuf> {
    fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.contains(".skac-") && name.ends_with(".tmp"))
        })
        .collect()
}

mod artifact;
mod cli;
mod pipeline;
mod request;
mod toolchain;
