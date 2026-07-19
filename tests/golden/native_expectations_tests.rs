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
    let source = write_case(directory.path(), b"7\n", None);

    let expected = load_native_expectations(&source).unwrap();

    assert_eq!(expected.exit_code(), 7);
    assert_eq!(expected.stdout(), b"");
    assert!(verify_native_execution(&expected, Some(7), b"", b"").is_ok());
    assert!(verify_native_execution(&expected, Some(7), b"unexpected", b"").is_err());
}

#[test]
fn stdout_sidecar_is_loaded_and_compared_as_exact_bytes() {
    let directory = TemporaryDirectory::new();
    let source = write_case(directory.path(), b"0\n", Some(b"42\r\n\xff"));

    let expected = load_native_expectations(&source).unwrap();

    assert_eq!(expected.stdout(), b"42\r\n\xff");
    assert!(verify_native_execution(&expected, Some(0), b"42\r\n\xff", b"").is_ok());
}

#[test]
fn missing_trailing_line_feed_is_reported_unambiguously() {
    let directory = TemporaryDirectory::new();
    let source = write_case(directory.path(), b"0", Some(b"42\n"));
    let expected = load_native_expectations(&source).unwrap();

    let error = verify_native_execution(&expected, Some(0), b"42", b"").unwrap_err();

    assert!(error.contains("expected (3 bytes): b\"42\\n\""));
    assert!(error.contains("actual (2 bytes): b\"42\""));
}

#[test]
fn extra_trailing_line_feed_is_reported_unambiguously() {
    let directory = TemporaryDirectory::new();
    let source = write_case(directory.path(), b"0", Some(b"42"));
    let expected = load_native_expectations(&source).unwrap();

    let error = verify_native_execution(&expected, Some(0), b"42\n", b"").unwrap_err();

    assert!(error.contains("expected (2 bytes): b\"42\""));
    assert!(error.contains("actual (3 bytes): b\"42\\n\""));
}

#[test]
fn non_utf8_mismatch_uses_escaped_byte_spelling() {
    let directory = TemporaryDirectory::new();
    let source = write_case(directory.path(), b"0", Some(b"\xff\n"));
    let expected = load_native_expectations(&source).unwrap();

    let error = verify_native_execution(&expected, Some(0), b"\xfe\n", b"").unwrap_err();

    assert!(error.contains("expected (2 bytes): b\"\\xff\\n\""));
    assert!(error.contains("actual (2 bytes): b\"\\xfe\\n\""));
}

#[test]
fn exit_and_stderr_mismatches_are_reported_together() {
    let directory = TemporaryDirectory::new();
    let source = write_case(directory.path(), b"5", None);
    let expected = load_native_expectations(&source).unwrap();

    let error = verify_native_execution(&expected, Some(6), b"", b"problem\n").unwrap_err();

    assert!(error.contains("exit status mismatch: expected 5, found 6"));
    assert!(error.contains("runtime stderr was not empty (8 bytes): b\"problem\\n\""));
}

fn write_case(directory: &Path, exit: &[u8], stdout: Option<&[u8]>) -> PathBuf {
    let source = directory.join("case.ska");
    fs::write(source.with_extension("exit"), exit).unwrap();
    if let Some(stdout) = stdout {
        fs::write(source.with_extension("stdout"), stdout).unwrap();
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
