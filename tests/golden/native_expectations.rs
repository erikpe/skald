//! Loading and comparison of native golden-test expectations.

use std::{fs, io::ErrorKind, path::Path};

#[derive(Debug, Eq, PartialEq)]
pub struct NativeExpectations {
    exit_status: ExpectedExitStatus,
    stdin: Vec<u8>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

impl NativeExpectations {
    pub fn stdin(&self) -> &[u8] {
        &self.stdin
    }

    pub fn stdout(&self) -> &[u8] {
        &self.stdout
    }

    pub fn stderr(&self) -> &[u8] {
        &self.stderr
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExpectedExitStatus {
    Code(i32),
    Failure,
}

/// Loads the required `.exit` sidecar and optional exact output sidecars.
///
/// An exit sidecar contains either one exact status in `0..=255` or `failure`
/// when the contract promises only unsuccessful termination. A missing
/// `.stdin`, `.stdout`, or `.stderr` sidecar means the corresponding stream
/// must be empty.
pub fn load_native_expectations(source: &Path) -> Result<NativeExpectations, String> {
    let exit_path = source.with_extension("exit");
    let exit_text = fs::read_to_string(&exit_path)
        .map_err(|error| format!("could not read {}: {error}", exit_path.display()))?;
    let exit_status = parse_exit_status(exit_text.trim())?;

    let stdin = read_optional_sidecar(source, "stdin")?;
    let stdout = read_optional_sidecar(source, "stdout")?;
    let stderr = read_optional_sidecar(source, "stderr")?;

    Ok(NativeExpectations {
        exit_status,
        stdin,
        stdout,
        stderr,
    })
}

fn read_optional_sidecar(source: &Path, extension: &str) -> Result<Vec<u8>, String> {
    let path = source.with_extension(extension);
    match fs::read(&path) {
        Ok(bytes) => Ok(bytes),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(format!("could not read {}: {error}", path.display())),
    }
}

fn parse_exit_status(text: &str) -> Result<ExpectedExitStatus, String> {
    if text == "failure" {
        return Ok(ExpectedExitStatus::Failure);
    }

    let code = text
        .parse::<i32>()
        .map_err(|error| format!("invalid expected exit status: {error}"))?;
    if !(0..=255).contains(&code) {
        return Err(format!("expected exit status {code} is outside 0..=255"));
    }
    Ok(ExpectedExitStatus::Code(code))
}

/// Checks all process-level expectations so one failed case reports every
/// independently observable mismatch at once.
pub fn verify_native_execution(
    expected: &NativeExpectations,
    actual_exit_code: Option<i32>,
    actual_stdout: &[u8],
    actual_stderr: &[u8],
) -> Result<(), String> {
    let mut mismatches = Vec::new();

    match (expected.exit_status, actual_exit_code) {
        (ExpectedExitStatus::Code(expected), Some(actual)) if actual != expected => mismatches
            .push(format!(
                "exit status mismatch: expected {expected}, found {actual}"
            )),
        (ExpectedExitStatus::Code(_), None) => {
            mismatches.push("generated executable terminated by signal".to_owned());
        }
        (ExpectedExitStatus::Failure, Some(0)) => {
            mismatches.push("expected unsuccessful termination, found exit status 0".to_owned());
        }
        _ => {}
    }

    if actual_stdout != expected.stdout() {
        mismatches.push(format!(
            "stdout mismatch\nexpected ({} bytes): {}\nactual ({} bytes): {}",
            expected.stdout().len(),
            display_bytes(expected.stdout()),
            actual_stdout.len(),
            display_bytes(actual_stdout)
        ));
    }

    if actual_stderr != expected.stderr() {
        mismatches.push(format!(
            "stderr mismatch\nexpected ({} bytes): {}\nactual ({} bytes): {}",
            expected.stderr().len(),
            display_bytes(expected.stderr()),
            actual_stderr.len(),
            display_bytes(actual_stderr)
        ));
    }

    if mismatches.is_empty() {
        Ok(())
    } else {
        Err(mismatches.join("\n"))
    }
}

fn display_bytes(bytes: &[u8]) -> String {
    let mut output = String::from("b\"");
    for byte in bytes {
        for escaped in std::ascii::escape_default(*byte) {
            output.push(char::from(escaped));
        }
    }
    output.push('"');
    output
}
