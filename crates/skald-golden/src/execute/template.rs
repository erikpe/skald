use super::ExecutionError;
use std::{collections::BTreeMap, ffi::OsString, path::PathBuf};

pub(super) struct TemporaryPaths {
    values: BTreeMap<String, PathBuf>,
}

impl TemporaryPaths {
    pub(super) fn new(sandbox: &std::path::Path, names: impl IntoIterator<Item = String>) -> Self {
        Self {
            values: names
                .into_iter()
                .map(|name| {
                    let path = sandbox.join(&name);
                    (name, path)
                })
                .collect(),
        }
    }

    pub(super) fn path(&self, name: &str) -> &std::path::Path {
        self.values
            .get(name)
            .expect("validated temporary name must have a path")
    }

    pub(super) fn substitute(&self, bytes: &[u8]) -> Result<Vec<u8>, ExecutionError> {
        let mut output = Vec::with_capacity(bytes.len());
        let mut remaining = bytes;
        while let Some(start) = find(remaining, b"{tmp:") {
            output.extend_from_slice(&remaining[..start]);
            let placeholder = &remaining[start..];
            let Some(end) = placeholder.iter().position(|byte| *byte == b'}') else {
                return Err(ExecutionError::plain(
                    "unterminated temporary-path placeholder",
                ));
            };
            let name = std::str::from_utf8(&placeholder[5..end])
                .map_err(|_| ExecutionError::plain("temporary-path name is not UTF-8"))?;
            let path = self.values.get(name).ok_or_else(|| {
                ExecutionError::plain(format!("unknown temporary-path placeholder {name:?}"))
            })?;
            output.extend_from_slice(&path_bytes(path));
            remaining = &placeholder[end + 1..];
        }
        output.extend_from_slice(remaining);
        Ok(output)
    }

    pub(super) fn substitute_argument(
        &self,
        argument: &std::ffi::OsStr,
    ) -> Result<OsString, ExecutionError> {
        argument_from_bytes(self.substitute(&argument_bytes(argument))?)
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[cfg(unix)]
fn path_bytes(path: &std::path::Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    path.as_os_str().as_bytes().to_vec()
}

#[cfg(not(unix))]
fn path_bytes(path: &std::path::Path) -> Vec<u8> {
    path.to_string_lossy().as_bytes().to_vec()
}

#[cfg(unix)]
fn argument_bytes(argument: &std::ffi::OsStr) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;
    argument.as_bytes().to_vec()
}

#[cfg(not(unix))]
fn argument_bytes(argument: &std::ffi::OsStr) -> Vec<u8> {
    argument.to_string_lossy().as_bytes().to_vec()
}

#[cfg(unix)]
fn argument_from_bytes(bytes: Vec<u8>) -> Result<OsString, ExecutionError> {
    use std::os::unix::ffi::OsStringExt;
    Ok(OsString::from_vec(bytes))
}

#[cfg(not(unix))]
fn argument_from_bytes(bytes: Vec<u8>) -> Result<OsString, ExecutionError> {
    String::from_utf8(bytes)
        .map(OsString::from)
        .map_err(|_| ExecutionError::plain("substituted argument is not UTF-8"))
}
