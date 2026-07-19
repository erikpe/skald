//! Loading and comparison of native golden-test expectations.

use std::{fs, io::ErrorKind, path::Path};

#[derive(Debug, Eq, PartialEq)]
pub struct NativeExpectations {
    exit_code: i32,
    stdout: Vec<u8>,
}

impl NativeExpectations {
    pub const fn exit_code(&self) -> i32 {
        self.exit_code
    }

    pub fn stdout(&self) -> &[u8] {
        &self.stdout
    }
}

/// Loads the required `.exit` sidecar and optional exact `.stdout` sidecar for
/// a native golden source. A missing stdout sidecar means empty stdout.
pub fn load_native_expectations(source: &Path) -> Result<NativeExpectations, String> {
    let exit_path = source.with_extension("exit");
    let exit_text = fs::read_to_string(&exit_path)
        .map_err(|error| format!("could not read {}: {error}", exit_path.display()))?;
    let exit_code = exit_text
        .trim()
        .parse::<i32>()
        .map_err(|error| format!("invalid expected exit status: {error}"))?;
    if !(0..=255).contains(&exit_code) {
        return Err(format!(
            "expected exit status {exit_code} is outside 0..=255"
        ));
    }

    let stdout_path = source.with_extension("stdout");
    let stdout = match fs::read(&stdout_path) {
        Ok(stdout) => stdout,
        Err(error) if error.kind() == ErrorKind::NotFound => Vec::new(),
        Err(error) => {
            return Err(format!("could not read {}: {error}", stdout_path.display()));
        }
    };

    Ok(NativeExpectations { exit_code, stdout })
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

    match actual_exit_code {
        Some(actual) if actual != expected.exit_code() => mismatches.push(format!(
            "exit status mismatch: expected {}, found {actual}",
            expected.exit_code()
        )),
        None => mismatches.push("generated executable terminated by signal".to_owned()),
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

    if !actual_stderr.is_empty() {
        mismatches.push(format!(
            "runtime stderr was not empty ({} bytes): {}",
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
