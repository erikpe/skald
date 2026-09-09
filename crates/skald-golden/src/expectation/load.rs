use super::ExpectationError;
use crate::{ResolvedArgs, ResolvedByteSource};
use std::{
    ffi::OsString,
    fs::{self, File},
    io::{self, Read},
    path::Path,
};

pub(crate) enum BoundedBytes {
    Complete(Vec<u8>),
    Overflow(Vec<u8>),
}

/// Loads inline UTF-8 or an external file without changing any byte.
pub fn load_bytes(source: &ResolvedByteSource) -> Result<Vec<u8>, ExpectationError> {
    match source {
        ResolvedByteSource::Inline(contents) => Ok(contents.as_bytes().to_vec()),
        ResolvedByteSource::File(path) => fs::read(path).map_err(|source| {
            ExpectationError::io(path.clone(), "could not read exact-byte data", source)
        }),
    }
}

pub(crate) fn load_bytes_bounded(
    source: &ResolvedByteSource,
    limit: usize,
) -> Result<BoundedBytes, ExpectationError> {
    match source {
        ResolvedByteSource::Inline(contents) => {
            Ok(bound_bytes(contents.as_bytes().iter().copied(), limit))
        }
        ResolvedByteSource::File(path) => read_bytes_bounded(path, limit).map_err(|source| {
            ExpectationError::io(path.clone(), "could not read exact-byte data", source)
        }),
    }
}

pub(crate) fn read_bytes_bounded(path: &Path, limit: usize) -> io::Result<BoundedBytes> {
    let file = File::open(path)?;
    read_bounded(file, limit)
}

fn read_bounded(mut input: impl Read, limit: usize) -> io::Result<BoundedBytes> {
    let mut bytes = Vec::with_capacity(limit.min(8 * 1024));
    let mut buffer = [0u8; 8 * 1024];
    loop {
        let count = input.read(&mut buffer)?;
        if count == 0 {
            return Ok(BoundedBytes::Complete(bytes));
        }
        let retained = limit.saturating_sub(bytes.len()).min(count);
        bytes.extend_from_slice(&buffer[..retained]);
        if retained < count {
            return Ok(BoundedBytes::Overflow(bytes));
        }
    }
}

fn bound_bytes(bytes: impl Iterator<Item = u8>, limit: usize) -> BoundedBytes {
    let mut bytes = bytes.take(limit.saturating_add(1)).collect::<Vec<_>>();
    if bytes.len() > limit {
        bytes.truncate(limit);
        BoundedBytes::Overflow(bytes)
    } else {
        BoundedBytes::Complete(bytes)
    }
}

/// Decodes UTF-8 arguments or the NUL-terminated exact-byte argument format.
pub fn decode_arguments(source: &ResolvedArgs) -> Result<Vec<OsString>, ExpectationError> {
    match source {
        ResolvedArgs::Utf8(arguments) => Ok(arguments.iter().map(OsString::from).collect()),
        ResolvedArgs::File(path) => {
            let bytes = fs::read(path).map_err(|source| {
                ExpectationError::io(path.clone(), "could not read exact-byte arguments", source)
            })?;
            decode_argument_bytes(path, &bytes)
        }
    }
}

#[cfg(unix)]
fn decode_argument_bytes(
    path: &std::path::Path,
    bytes: &[u8],
) -> Result<Vec<OsString>, ExpectationError> {
    use std::os::unix::ffi::OsStringExt;

    if bytes.is_empty() {
        return Ok(Vec::new());
    }
    if bytes.last() != Some(&0) {
        return Err(ExpectationError::invalid(
            path.to_path_buf(),
            "nonempty exact-byte argument file must end with NUL",
        ));
    }
    Ok(bytes[..bytes.len() - 1]
        .split(|byte| *byte == 0)
        .map(|argument| OsString::from_vec(argument.to_vec()))
        .collect())
}

#[cfg(not(unix))]
fn decode_argument_bytes(
    path: &std::path::Path,
    _bytes: &[u8],
) -> Result<Vec<OsString>, ExpectationError> {
    Err(ExpectationError::invalid(
        path.to_path_buf(),
        "exact-byte argument files are supported only on Unix",
    ))
}
