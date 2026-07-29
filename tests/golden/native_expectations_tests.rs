#[path = "native_expectations.rs"]
mod native_expectations;

use std::{
    fs, io,
    path::{Path, PathBuf},
    process,
    sync::atomic::{AtomicUsize, Ordering},
};

use native_expectations::{load_native_expectations, verify_native_execution};

static NEXT_TEMPORARY_DIRECTORY: AtomicUsize = AtomicUsize::new(0);

#[test]
fn missing_stdout_sidecar_means_empty_output() {
    let directory = TemporaryDirectory::new();
    let source = write_case(directory.path(), b"7\n", None, None);

    let expected = load_native_expectations(&source).unwrap();

    assert_eq!(expected.stdout(), b"");
    assert!(verify_native_execution(&expected, Some(7), b"", b"").is_ok());
    assert!(verify_native_execution(&expected, Some(7), b"unexpected", b"").is_err());
}

#[test]
fn failure_accepts_a_nonzero_status_or_signal_without_freezing_either() {
    let directory = TemporaryDirectory::new();
    let source = write_case(directory.path(), b"failure\n", None, None);
    let expected = load_native_expectations(&source).unwrap();

    assert!(verify_native_execution(&expected, Some(1), b"", b"").is_ok());
    assert!(verify_native_execution(&expected, None, b"", b"").is_ok());

    let error = verify_native_execution(&expected, Some(0), b"", b"").unwrap_err();
    assert!(error.contains("expected unsuccessful termination, found exit status 0"));
}

#[test]
fn stdout_sidecar_is_loaded_and_compared_as_exact_bytes() {
    let directory = TemporaryDirectory::new();
    let source = write_case(directory.path(), b"0\n", Some(b"42\r\n\xff"), None);

    let expected = load_native_expectations(&source).unwrap();

    assert_eq!(expected.stdout(), b"42\r\n\xff");
    assert!(verify_native_execution(&expected, Some(0), b"42\r\n\xff", b"").is_ok());
}

#[test]
fn missing_trailing_line_feed_is_reported_unambiguously() {
    let directory = TemporaryDirectory::new();
    let source = write_case(directory.path(), b"0", Some(b"42\n"), None);
    let expected = load_native_expectations(&source).unwrap();

    let error = verify_native_execution(&expected, Some(0), b"42", b"").unwrap_err();

    assert!(error.contains("expected (3 bytes): b\"42\\n\""));
    assert!(error.contains("actual (2 bytes): b\"42\""));
}

#[test]
fn extra_trailing_line_feed_is_reported_unambiguously() {
    let directory = TemporaryDirectory::new();
    let source = write_case(directory.path(), b"0", Some(b"42"), None);
    let expected = load_native_expectations(&source).unwrap();

    let error = verify_native_execution(&expected, Some(0), b"42\n", b"").unwrap_err();

    assert!(error.contains("expected (2 bytes): b\"42\""));
    assert!(error.contains("actual (3 bytes): b\"42\\n\""));
}

#[test]
fn non_utf8_mismatch_uses_escaped_byte_spelling() {
    let directory = TemporaryDirectory::new();
    let source = write_case(directory.path(), b"0", Some(b"\xff\n"), None);
    let expected = load_native_expectations(&source).unwrap();

    let error = verify_native_execution(&expected, Some(0), b"\xfe\n", b"").unwrap_err();

    assert!(error.contains("expected (2 bytes): b\"\\xff\\n\""));
    assert!(error.contains("actual (2 bytes): b\"\\xfe\\n\""));
}

#[test]
fn missing_stderr_sidecar_means_empty_output() {
    let directory = TemporaryDirectory::new();
    let source = write_case(directory.path(), b"0", None, None);
    let expected = load_native_expectations(&source).unwrap();

    assert_eq!(expected.stderr(), b"");
    assert!(verify_native_execution(&expected, Some(0), b"", b"").is_ok());

    let error = verify_native_execution(&expected, Some(0), b"", b"unexpected").unwrap_err();
    assert!(error.contains("expected (0 bytes): b\"\""));
    assert!(error.contains("actual (10 bytes): b\"unexpected\""));
}

#[test]
fn stderr_sidecar_is_loaded_and_compared_as_exact_bytes() {
    let directory = TemporaryDirectory::new();
    let source = write_case(
        directory.path(),
        b"failure",
        None,
        Some(b"panic: bad\x00input\n"),
    );
    let expected = load_native_expectations(&source).unwrap();

    assert_eq!(expected.stderr(), b"panic: bad\x00input\n");
    assert!(verify_native_execution(&expected, Some(1), b"", b"panic: bad\x00input\n").is_ok());
}

#[test]
fn stderr_missing_and_extra_bytes_are_reported_unambiguously() {
    let directory = TemporaryDirectory::new();
    let source = write_case(directory.path(), b"failure", None, Some(b"panic: bad\n"));
    let expected = load_native_expectations(&source).unwrap();

    let missing = verify_native_execution(&expected, Some(1), b"", b"panic: bad").unwrap_err();
    assert!(missing.contains("expected (11 bytes): b\"panic: bad\\n\""));
    assert!(missing.contains("actual (10 bytes): b\"panic: bad\""));

    let extra = verify_native_execution(&expected, Some(1), b"", b"panic: bad\n\n").unwrap_err();
    assert!(extra.contains("actual (12 bytes): b\"panic: bad\\n\\n\""));
}

#[test]
fn non_utf8_stderr_mismatch_uses_escaped_byte_spelling() {
    let directory = TemporaryDirectory::new();
    let source = write_case(directory.path(), b"failure", None, Some(b"\xff\n"));
    let expected = load_native_expectations(&source).unwrap();

    let error = verify_native_execution(&expected, Some(1), b"", b"\xfe\n").unwrap_err();

    assert!(error.contains("expected (2 bytes): b\"\\xff\\n\""));
    assert!(error.contains("actual (2 bytes): b\"\\xfe\\n\""));
}

#[test]
fn exit_stdout_and_stderr_mismatches_are_reported_together() {
    let directory = TemporaryDirectory::new();
    let source = write_case(
        directory.path(),
        b"5",
        Some(b"expected stdout\n"),
        Some(b"expected stderr\n"),
    );
    let expected = load_native_expectations(&source).unwrap();

    let error = verify_native_execution(&expected, Some(6), b"actual stdout\n", b"actual stderr\n")
        .unwrap_err();

    assert!(error.contains("exit status mismatch: expected 5, found 6"));
    assert!(error.contains("stdout mismatch"));
    assert!(error.contains("stderr mismatch"));
}

fn write_case(
    directory: &Path,
    exit: &[u8],
    stdout: Option<&[u8]>,
    stderr: Option<&[u8]>,
) -> PathBuf {
    let source = directory.join("case.ska");
    fs::write(source.with_extension("exit"), exit).unwrap();
    if let Some(stdout) = stdout {
        fs::write(source.with_extension("stdout"), stdout).unwrap();
    }
    if let Some(stderr) = stderr {
        fs::write(source.with_extension("stderr"), stderr).unwrap();
    }
    source
}

struct TemporaryDirectory(PathBuf);

impl TemporaryDirectory {
    fn new() -> Self {
        loop {
            let sequence = NEXT_TEMPORARY_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "skald-golden-expectations-{}-{sequence}",
                process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!(
                    "could not create temporary directory {}: {error}",
                    path.display()
                ),
            }
        }
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        if let Err(error) = fs::remove_dir_all(&self.0) {
            if error.kind() != io::ErrorKind::NotFound && !std::thread::panicking() {
                panic!(
                    "could not remove temporary directory {}: {error}",
                    self.0.display()
                );
            }
        }
    }
}
