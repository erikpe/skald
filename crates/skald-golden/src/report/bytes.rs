use crate::ProcessCommand;
use std::{ffi::OsStr, fmt::Write as _, path::Path};

const MAX_DIFF_BYTES: usize = 8 * 1024;
const MAX_ESCAPED_BYTES: usize = 512;

pub(crate) fn escape_command(command: &ProcessCommand) -> String {
    std::iter::once(command.program().as_os_str())
        .chain(command.arguments().iter().map(|value| value.as_os_str()))
        .map(escape_os)
        .collect::<Vec<_>>()
        .join(" ")
}

pub(crate) fn escape_path(path: &Path) -> String {
    escape_os(path.as_os_str())
}

#[cfg(unix)]
fn os_bytes(value: &OsStr) -> &[u8] {
    use std::os::unix::ffi::OsStrExt;
    value.as_bytes()
}

#[cfg(not(unix))]
fn os_bytes(value: &OsStr) -> &[u8] {
    value.to_str().unwrap_or("<non-UTF-8 path>").as_bytes()
}

fn escape_os(value: &OsStr) -> String {
    format!(
        "b\"{}\"",
        escape_bytes_with_limit(os_bytes(value), usize::MAX)
    )
}

pub(crate) fn escape_bytes(bytes: &[u8]) -> String {
    escape_bytes_with_limit(bytes, MAX_ESCAPED_BYTES)
}

fn escape_bytes_with_limit(bytes: &[u8], limit: usize) -> String {
    let mut output = String::new();
    for byte in bytes.iter().take(limit) {
        match byte {
            b'\\' => output.push_str("\\\\"),
            b'\"' => output.push_str("\\\""),
            b'\n' => output.push_str("\\n"),
            b'\r' => output.push_str("\\r"),
            b'\t' => output.push_str("\\t"),
            0x20..=0x7e => output.push(char::from(*byte)),
            _ => write!(output, "\\x{byte:02x}").expect("writing to a string cannot fail"),
        }
    }
    if bytes.len() > limit {
        write!(output, "…<{} bytes omitted>", bytes.len() - limit)
            .expect("writing to a string cannot fail");
    }
    output
}

pub(crate) fn diff(expected: &[u8], actual: &[u8]) -> Option<String> {
    let expected = std::str::from_utf8(expected).ok()?;
    let actual = std::str::from_utf8(actual).ok()?;
    if expected == actual {
        return None;
    }
    let mut output = String::from("--- expected\n+++ actual\n");
    for line in expected.lines() {
        output.push('-');
        output.push_str(line);
        output.push('\n');
        if output.len() >= MAX_DIFF_BYTES {
            output.truncate(MAX_DIFF_BYTES);
            output.push_str("\n... diff truncated ...\n");
            return Some(output);
        }
    }
    for line in actual.lines() {
        output.push('+');
        output.push_str(line);
        output.push('\n');
        if output.len() >= MAX_DIFF_BYTES {
            output.truncate(MAX_DIFF_BYTES);
            output.push_str("\n... diff truncated ...\n");
            return Some(output);
        }
    }
    Some(output)
}
