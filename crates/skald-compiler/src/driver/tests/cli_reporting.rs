use std::{
    io::{self, Write},
    os::unix::fs::PermissionsExt,
};

use super::*;

#[test]
fn phase_reports_cover_compilation_publication_artifact_and_driver_total() {
    let directory = TemporaryDirectory::new("driver-phase-reports").unwrap();
    let input = directory.join("main.ska");
    let output = directory.join("main.s");
    fs::write(&input, "fn main() -> i64 { return 42; }\n").unwrap();

    let (status, stdout, stderr) = run(&[
        "skac",
        input.to_str().unwrap(),
        "--no-stdlib",
        "--emit",
        "asm",
        "-o",
        output.to_str().unwrap(),
        "-v",
    ]);

    assert_eq!(status, 0, "{stderr}");
    assert!(stdout.is_empty());
    assert!(output.is_file());
    assert!(stderr.starts_with("skac: phase: provider normalization started\n"));
    assert!(stderr.contains("skac: run: compilation completed\n"));
    assert!(stderr.contains("skac: phase: artifact publication started\n"));
    assert!(stderr.contains("skac: phase: artifact publication completed\n"));
    assert!(stderr.contains(&format!("skac: artifact: assembly {}\n", output.display())));
    assert!(stderr.ends_with("skac: run: driver completed\n"));
    assert!(!stderr.contains("skac: phase: host linking"));
    assert!(!stderr.contains("skac: stats:"));
    assert!(!stderr.contains(" in "));
    assert_ordered(
        &stderr,
        &[
            "skac: run: compilation completed",
            "skac: phase: artifact publication started",
            "skac: phase: artifact publication completed",
            "skac: artifact: assembly",
            "skac: run: driver completed",
        ],
    );
}

#[test]
fn details_and_trace_add_only_their_owned_information() {
    let directory = TemporaryDirectory::new("driver-detailed-reports").unwrap();
    let input = directory.join("main.ska");
    fs::write(&input, "fn main() -> i64 { return 42; }\n").unwrap();

    let details_output = directory.join("details.s");
    let (status, stdout, details) = run(&[
        "skac",
        input.to_str().unwrap(),
        "--no-stdlib",
        "--emit",
        "asm",
        "-o",
        details_output.to_str().unwrap(),
        "--report-level",
        "details",
    ]);
    assert_eq!(status, 0, "{details}");
    assert!(stdout.is_empty());
    assert!(details.contains("skac: phase: module loading completed in "));
    assert!(details.contains("skac: stats: discovery parse executions: 1\n"));
    assert!(details.contains("skac: run: compilation completed in "));
    assert!(details.contains("skac: run: driver completed in "));
    assert!(!details.contains("skac: trace:"));

    let trace_output = directory.join("trace.s");
    let (status, _, trace) = run(&[
        "skac",
        input.to_str().unwrap(),
        "--no-stdlib",
        "--emit",
        "asm",
        "-o",
        trace_output.to_str().unwrap(),
        "-vvv",
    ]);
    assert_eq!(status, 0, "{trace}");
    assert!(trace.contains("skac: trace: discovery parsed module main:"));
    assert!(trace.contains("skac: trace: final parsed module main:"));
}

#[test]
fn executable_reports_linking_then_publication_and_notices_the_final_artifact() {
    let directory = TemporaryDirectory::new("driver-executable-reports").unwrap();
    let input = directory.join("main.ska");
    let output = directory.join("main");
    fs::write(&input, "fn main() -> i64 { return 42; }\n").unwrap();
    let toolchain = fake_toolchain(&directory);

    let (status, stdout, stderr) = run_with_toolchain(
        &[
            "skac",
            input.to_str().unwrap(),
            "--no-stdlib",
            "-o",
            output.to_str().unwrap(),
            "-v",
        ],
        &toolchain,
    );

    assert_eq!(status, 0, "{stderr}");
    assert!(stdout.is_empty());
    assert_eq!(fs::read_to_string(&output).unwrap(), "linked executable");
    assert_ordered(
        &stderr,
        &[
            "skac: run: compilation completed",
            "skac: phase: host linking started",
            "skac: phase: host linking completed",
            "skac: phase: artifact publication started",
            "skac: phase: artifact publication completed",
            "skac: artifact: executable",
            "skac: run: driver completed",
        ],
    );
}

#[test]
fn terminal_driver_failures_finish_reporting_before_the_existing_error() {
    let directory = TemporaryDirectory::new("driver-failed-reports").unwrap();
    let input = directory.join("main.ska");
    fs::write(&input, "fn main() -> i64 { return 42; }\n").unwrap();

    let missing_root = directory.join("missing-root");
    let (status, _, provider) = run(&[
        "skac",
        "--entry",
        "app",
        "--module-root",
        missing_root.to_str().unwrap(),
        "--no-stdlib",
        "--emit",
        "asm",
        "-v",
    ]);
    assert_eq!(status, EXIT_COMPILE_ERROR);
    assert_ordered(
        &provider,
        &[
            "skac: phase: provider normalization started",
            "skac: phase: provider normalization failed",
            "skac: run: compilation failed",
            "skac: run: driver failed",
            "skac: cannot normalize provider root",
        ],
    );

    let runtime = directory.join("runtime.a");
    fs::write(&runtime, "runtime").unwrap();
    let executable = directory.join("main");
    let (status, _, linker) = run_with_toolchain(
        &[
            "skac",
            input.to_str().unwrap(),
            "--no-stdlib",
            "-o",
            executable.to_str().unwrap(),
            "-v",
        ],
        &Toolchain::new("false", runtime),
    );
    assert_eq!(status, EXIT_COMPILE_ERROR);
    assert_ordered(
        &linker,
        &[
            "skac: phase: host linking started",
            "skac: phase: host linking failed",
            "skac: run: driver failed",
            "skac: toolchain `false` failed with exit status 1",
        ],
    );
    assert_eq!(linker.matches("toolchain `false` failed").count(), 1);

    let publication = directory.join("existing-directory");
    fs::create_dir(&publication).unwrap();
    let (status, _, published) = run(&[
        "skac",
        input.to_str().unwrap(),
        "--no-stdlib",
        "--emit",
        "asm",
        "-o",
        publication.to_str().unwrap(),
        "-v",
    ]);
    assert_eq!(status, 74);
    assert_ordered(
        &published,
        &[
            "skac: phase: artifact publication started",
            "skac: phase: artifact publication failed",
            "skac: run: driver failed",
            "skac: could not publish assembly output:",
        ],
    );
    assert_eq!(
        published
            .matches("could not publish assembly output")
            .count(),
        1
    );
    assert!(!published.contains("skac: artifact:"));

    let executable_publication = directory.join("existing-executable-directory");
    fs::create_dir(&executable_publication).unwrap();
    let toolchain = fake_toolchain(&directory);
    let (status, _, published) = run_with_toolchain(
        &[
            "skac",
            input.to_str().unwrap(),
            "--no-stdlib",
            "-o",
            executable_publication.to_str().unwrap(),
            "-v",
        ],
        &toolchain,
    );
    assert_eq!(status, EXIT_COMPILE_ERROR);
    assert_ordered(
        &published,
        &[
            "skac: phase: host linking completed",
            "skac: phase: artifact publication started",
            "skac: phase: artifact publication failed",
            "skac: run: driver failed",
            "skac: could not publish linked executable:",
        ],
    );
    assert_eq!(
        published
            .matches("could not publish linked executable")
            .count(),
        1
    );
    assert!(!published.contains("skac: artifact:"));
}

#[test]
fn source_errors_remain_visible_at_both_diagnostic_levels_and_when_reports_are_off() {
    let directory = TemporaryDirectory::new("driver-diagnostic-levels").unwrap();
    let input = directory.join("broken.ska");
    fs::write(&input, "fn main() -> i64 { return missing; }\n").unwrap();

    for level in ["warning", "error"] {
        let (status, stdout, stderr) = run(&[
            "skac",
            input.to_str().unwrap(),
            "--no-stdlib",
            "--emit",
            "asm",
            "--diagnostic-level",
            level,
            "--report-level",
            "off",
        ]);
        assert_eq!(status, EXIT_COMPILE_ERROR, "{level}: {stderr}");
        assert!(stdout.is_empty());
        assert!(stderr.contains("error[RES003]"), "{level}: {stderr}");
        assert!(!stderr.contains("skac: phase:"), "{level}: {stderr}");
    }
}

#[test]
fn a_retained_report_writer_error_does_not_cancel_artifact_production() {
    let directory = TemporaryDirectory::new("driver-report-writer").unwrap();
    let input = directory.join("main.ska");
    let output = directory.join("main.s");
    fs::write(&input, "fn main() -> i64 { return 42; }\n").unwrap();
    let args = [
        OsString::from("skac"),
        input.into_os_string(),
        OsString::from("--no-stdlib"),
        OsString::from("--emit"),
        OsString::from("asm"),
        OsString::from("-o"),
        output.clone().into_os_string(),
        OsString::from("-v"),
    ];
    let mut stdout = Vec::new();
    let mut stderr = FailOnceWriter::default();

    let error = run_cli_with_context(
        args,
        &mut stdout,
        &mut stderr,
        &Toolchain::new("false", "missing-runtime.a"),
    )
    .unwrap_err();

    assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
    assert!(stdout.is_empty());
    assert!(output.is_file());
}

fn assert_ordered(text: &str, fragments: &[&str]) {
    let mut offset = 0usize;
    for fragment in fragments {
        let relative = text[offset..]
            .find(fragment)
            .unwrap_or_else(|| panic!("missing `{fragment}` after offset {offset}:\n{text}"));
        offset += relative + fragment.len();
    }
}

fn fake_toolchain(directory: &TemporaryDirectory) -> Toolchain {
    let runtime = directory.join("runtime.a");
    fs::write(&runtime, "runtime").unwrap();
    let linker = directory.join("fake-linker.sh");
    fs::write(
        &linker,
        concat!(
            "#!/bin/sh\n",
            "output=\n",
            "while [ \"$#\" -gt 0 ]; do\n",
            "  if [ \"$1\" = \"-o\" ]; then output=$2; shift 2; else shift; fi\n",
            "done\n",
            "cat >/dev/null\n",
            "printf 'linked executable' >\"$output\"\n",
        ),
    )
    .unwrap();
    let mut permissions = fs::metadata(&linker).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&linker, permissions).unwrap();
    Toolchain::new(linker.into_os_string(), runtime)
}

#[derive(Default)]
struct FailOnceWriter {
    failed: bool,
    bytes: Vec<u8>,
}

impl Write for FailOnceWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if !self.failed {
            self.failed = true;
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "injected report failure",
            ));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
