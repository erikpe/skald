#[path = "native_expectations.rs"]
mod native_expectations;

use std::{
    fs, io,
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
    process,
    sync::atomic::{AtomicUsize, Ordering},
};

use native_expectations::{load_native_expectations, verify_native_execution};

static NEXT_TEMPORARY_DIRECTORY: AtomicUsize = AtomicUsize::new(0);

#[test]
fn missing_argv_sidecar_means_no_additional_arguments() {
    let directory = TemporaryDirectory::new();
    let source = write_case(directory.path(), b"0\n", None, None, None);

    let expected = load_native_expectations(&source).unwrap();

    assert!(expected.arguments().is_empty());
}

#[test]
fn empty_argv_sidecar_means_no_additional_arguments() {
    let directory = TemporaryDirectory::new();
    let source = write_case(directory.path(), b"0\n", None, None, None);
    fs::write(source.with_extension("argv"), b"").unwrap();

    let expected = load_native_expectations(&source).unwrap();

    assert!(expected.arguments().is_empty());
}

#[test]
fn one_nul_in_argv_encodes_one_empty_argument() {
    let directory = TemporaryDirectory::new();
    let source = write_case(directory.path(), b"0\n", None, None, None);
    fs::write(source.with_extension("argv"), b"\0").unwrap();

    let expected = load_native_expectations(&source).unwrap();

    assert_eq!(argument_bytes(&expected), vec![b"".as_slice()]);
}

#[test]
fn argv_preserves_multiple_arguments_and_whitespace() {
    let directory = TemporaryDirectory::new();
    let source = write_case(directory.path(), b"0\n", None, None, None);
    fs::write(
        source.with_extension("argv"),
        b"plain\0space arg\0tab\targ\0line\nfeed\0",
    )
    .unwrap();

    let expected = load_native_expectations(&source).unwrap();

    assert_eq!(
        argument_bytes(&expected),
        vec![
            b"plain".as_slice(),
            b"space arg".as_slice(),
            b"tab\targ".as_slice(),
            b"line\nfeed".as_slice(),
        ]
    );
}

#[test]
fn argv_preserves_leading_consecutive_and_trailing_empty_arguments() {
    let directory = TemporaryDirectory::new();
    let source = write_case(directory.path(), b"0\n", None, None, None);
    fs::write(source.with_extension("argv"), b"\0middle\0\0").unwrap();

    let expected = load_native_expectations(&source).unwrap();

    assert_eq!(
        argument_bytes(&expected),
        vec![b"".as_slice(), b"middle".as_slice(), b"".as_slice()]
    );
}

#[test]
fn argv_preserves_non_utf8_bytes() {
    let directory = TemporaryDirectory::new();
    let source = write_case(directory.path(), b"0\n", None, None, None);
    fs::write(source.with_extension("argv"), b"before\xffafter\0").unwrap();

    let expected = load_native_expectations(&source).unwrap();

    assert_eq!(
        argument_bytes(&expected),
        vec![b"before\xffafter".as_slice()]
    );
}

#[test]
fn nonempty_argv_without_final_nul_is_rejected() {
    let directory = TemporaryDirectory::new();
    let source = write_case(directory.path(), b"0\n", None, None, None);
    let argv_path = source.with_extension("argv");
    fs::write(&argv_path, b"unterminated").unwrap();

    let error = load_native_expectations(&source).unwrap_err();

    assert_eq!(
        error,
        format!(
            "invalid executable argument sidecar {}: nonempty file must end with NUL",
            argv_path.display()
        )
    );
}

#[test]
fn case_argv_is_independent_of_compiler_case_args() {
    let directory = TemporaryDirectory::new();
    let arguments_manifest = directory.path().join("case.args");
    fs::write(&arguments_manifest, b"--entry\napp::main\n").unwrap();
    fs::write(arguments_manifest.with_extension("exit"), b"0\n").unwrap();

    let without_argv = load_native_expectations(&arguments_manifest).unwrap();
    assert!(without_argv.arguments().is_empty());

    fs::write(arguments_manifest.with_extension("argv"), b"runtime arg\0").unwrap();
    let with_argv = load_native_expectations(&arguments_manifest).unwrap();
    assert_eq!(argument_bytes(&with_argv), vec![b"runtime arg".as_slice()]);
}

#[test]
fn missing_stdin_sidecar_means_empty_input() {
    let directory = TemporaryDirectory::new();
    let source = write_case(directory.path(), b"0\n", None, None, None);

    let expected = load_native_expectations(&source).unwrap();

    assert_eq!(expected.stdin(), b"");
}

#[test]
fn binary_stdin_sidecar_is_loaded_byte_for_byte() {
    let directory = TemporaryDirectory::new();
    let source = write_case(
        directory.path(),
        b"0\n",
        Some(b"input\0\xff\n"),
        Some(b"input\0\xff\n"),
        None,
    );

    let expected = load_native_expectations(&source).unwrap();

    assert_eq!(expected.stdin(), b"input\0\xff\n");
    let error = verify_native_execution(&expected, Some(0), b"input\0\xfe\n", b"").unwrap_err();
    assert!(error.contains("expected (8 bytes): b\"input\\x00\\xff\\n\""));
    assert!(error.contains("actual (8 bytes): b\"input\\x00\\xfe\\n\""));
}

#[test]
fn missing_stdout_sidecar_means_empty_output() {
    let directory = TemporaryDirectory::new();
    let source = write_case(directory.path(), b"7\n", None, None, None);

    let expected = load_native_expectations(&source).unwrap();

    assert_eq!(expected.stdout(), b"");
    assert!(verify_native_execution(&expected, Some(7), b"", b"").is_ok());
    assert!(verify_native_execution(&expected, Some(7), b"unexpected", b"").is_err());
}

#[test]
fn failure_accepts_a_nonzero_status_or_signal_without_freezing_either() {
    let directory = TemporaryDirectory::new();
    let source = write_case(directory.path(), b"failure\n", None, None, None);
    let expected = load_native_expectations(&source).unwrap();

    assert!(verify_native_execution(&expected, Some(1), b"", b"").is_ok());
    assert!(verify_native_execution(&expected, None, b"", b"").is_ok());

    let error = verify_native_execution(&expected, Some(0), b"", b"").unwrap_err();
    assert!(error.contains("expected unsuccessful termination, found exit status 0"));
}

#[test]
fn stdout_sidecar_is_loaded_and_compared_as_exact_bytes() {
    let directory = TemporaryDirectory::new();
    let source = write_case(directory.path(), b"0\n", None, Some(b"42\r\n\xff"), None);

    let expected = load_native_expectations(&source).unwrap();

    assert_eq!(expected.stdout(), b"42\r\n\xff");
    assert!(verify_native_execution(&expected, Some(0), b"42\r\n\xff", b"").is_ok());
}

#[test]
fn missing_trailing_line_feed_is_reported_unambiguously() {
    let directory = TemporaryDirectory::new();
    let source = write_case(directory.path(), b"0", None, Some(b"42\n"), None);
    let expected = load_native_expectations(&source).unwrap();

    let error = verify_native_execution(&expected, Some(0), b"42", b"").unwrap_err();

    assert!(error.contains("expected (3 bytes): b\"42\\n\""));
    assert!(error.contains("actual (2 bytes): b\"42\""));
}

#[test]
fn extra_trailing_line_feed_is_reported_unambiguously() {
    let directory = TemporaryDirectory::new();
    let source = write_case(directory.path(), b"0", None, Some(b"42"), None);
    let expected = load_native_expectations(&source).unwrap();

    let error = verify_native_execution(&expected, Some(0), b"42\n", b"").unwrap_err();

    assert!(error.contains("expected (2 bytes): b\"42\""));
    assert!(error.contains("actual (3 bytes): b\"42\\n\""));
}

#[test]
fn non_utf8_mismatch_uses_escaped_byte_spelling() {
    let directory = TemporaryDirectory::new();
    let source = write_case(directory.path(), b"0", None, Some(b"\xff\n"), None);
    let expected = load_native_expectations(&source).unwrap();

    let error = verify_native_execution(&expected, Some(0), b"\xfe\n", b"").unwrap_err();

    assert!(error.contains("expected (2 bytes): b\"\\xff\\n\""));
    assert!(error.contains("actual (2 bytes): b\"\\xfe\\n\""));
}

#[test]
fn missing_stderr_sidecar_means_empty_output() {
    let directory = TemporaryDirectory::new();
    let source = write_case(directory.path(), b"0", None, None, None);
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
    let source = write_case(
        directory.path(),
        b"failure",
        None,
        None,
        Some(b"panic: bad\n"),
    );
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
    let source = write_case(directory.path(), b"failure", None, None, Some(b"\xff\n"));
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
        None,
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

fn argument_bytes(expected: &native_expectations::NativeExpectations) -> Vec<&[u8]> {
    expected
        .arguments()
        .iter()
        .map(|argument| argument.as_os_str().as_bytes())
        .collect()
}

fn write_case(
    directory: &Path,
    exit: &[u8],
    stdin: Option<&[u8]>,
    stdout: Option<&[u8]>,
    stderr: Option<&[u8]>,
) -> PathBuf {
    let source = directory.join("case.ska");
    fs::write(source.with_extension("exit"), exit).unwrap();
    if let Some(stdin) = stdin {
        fs::write(source.with_extension("stdin"), stdin).unwrap();
    }
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
