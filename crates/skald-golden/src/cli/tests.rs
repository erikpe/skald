use super::{options::Options, run_cli_with_context, stage_options, HELP};
use crate::{Determinism, ReportFormat, SandboxRetention};
use std::{
    fs,
    io::{self, Write},
    path::Path,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "skald-golden-cli-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("config.toml"), "schema = 1\n").unwrap();
        fs::write(root.join("program.ska"), "fn main() -> i64 { return 0; }\n").unwrap();
        fs::write(
            root.join("simple.golden.toml"),
            "schema=1\n[[test]]\nname='simple'\nmode='run'\nsource='program.ska'\n[[test.run]]\nname='default'\n",
        )
        .unwrap();
        Self { root }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

#[test]
fn parses_compiler_and_determinism_execution_options() {
    let options = Options::parse(
        [
            "skald-golden",
            "--compiler",
            "tools/skac",
            "--determinism",
            "full",
            "--jobs",
            "3",
            "--fail-fast",
            "--timeout",
            "17",
            "--show-output",
            "--slowest",
            "4",
            "--format",
            "json",
            "--keep-all-artifacts",
        ]
        .map(Into::into),
    )
    .unwrap();

    assert_eq!(options.compiler, Some(PathBuf::from("tools/skac")));
    assert_eq!(options.determinism, Determinism::Full);
    assert_eq!(options.jobs.unwrap().get(), 3);
    assert!(options.fail_fast);
    assert_eq!(options.timeout.unwrap().as_secs(), 17);
    assert!(options.show_output);
    assert_eq!(options.slowest.unwrap().get(), 4);
    assert_eq!(options.format, ReportFormat::Json);
    assert!(options.keep_all_artifacts);
}

#[test]
fn defaults_compiler_linker_and_execution_stages_to_sixty_seconds() {
    let options = stage_options(
        PathBuf::from("skac"),
        Path::new("."),
        Determinism::Off,
        None,
        SandboxRetention::Failures,
    );

    assert_eq!(options.compiler().default_timeout().as_secs(), 60);
    assert_eq!(options.linker_timeout().as_secs(), 60);
    assert_eq!(options.execution().default_timeout().as_secs(), 60);
    assert_eq!(options.runtime().command().timeout().as_secs(), 120);
}

#[test]
fn rejects_unknown_determinism_modes() {
    let error = Options::parse(["skald-golden", "--determinism", "sometimes"].map(Into::into))
        .err()
        .unwrap();

    assert_eq!(
        error,
        "unknown determinism mode \"sometimes\"; expected off, compile, or full"
    );
}

#[test]
fn rejects_zero_or_non_numeric_job_limits() {
    for value in ["0", "many"] {
        let error = Options::parse(["skald-golden", "--jobs", value].map(Into::into))
            .err()
            .unwrap();
        assert_eq!(error, "--jobs requires a positive integer");
    }
}

#[test]
fn rejects_invalid_timeout_slowest_and_report_format_values() {
    for (option, value, expected) in [
        (
            "--timeout",
            "0",
            "--timeout requires a positive integer number of seconds",
        ),
        ("--slowest", "none", "--slowest requires a positive integer"),
        (
            "--format",
            "yaml",
            "unknown report format \"yaml\"; expected human, json, or junit",
        ),
    ] {
        let error = Options::parse(["skald-golden", option, value].map(Into::into))
            .err()
            .unwrap();
        assert_eq!(error, expected);
    }
}

#[test]
fn help_documents_the_complete_frozen_surface() {
    for option in [
        "--jobs",
        "--filter",
        "--exclude",
        "--exact",
        "--variant",
        "--compiler",
        "--compiler-arg",
        "--determinism",
        "--timeout",
        "--fail-fast",
        "--list",
        "--list-tests",
        "--explain",
        "--show-output",
        "--slowest",
        "--format",
        "--allow-empty",
        "--keep-all-artifacts",
    ] {
        assert!(HELP.contains(option), "help omitted {option}");
    }
}

#[test]
fn read_only_cli_operations_render_the_validated_plan() {
    let fixture = Fixture::new();
    let artifact_root = fixture.root.with_extension("artifacts");
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_cli_with_context(
        ["skald-golden", "--list"].map(Into::into),
        &fixture.root,
        &artifact_root,
        &fixture.root,
        &mut stdout,
        &mut stderr,
    );
    assert_eq!(status, 0);
    assert_eq!(
        String::from_utf8(stdout).unwrap(),
        "simple::simple::default::default\n"
    );
    assert!(stderr.is_empty());
    assert!(!artifact_root.exists());
}

#[test]
fn empty_execution_does_not_require_a_compiler_or_prepare_artifacts() {
    let fixture = Fixture::new();
    let artifact_root = fixture.root.with_extension("artifacts");
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_cli_with_context(
        ["skald-golden", "--filter", "absent/**", "--allow-empty"].map(Into::into),
        &fixture.root,
        &artifact_root,
        &fixture.root,
        &mut stdout,
        &mut stderr,
    );
    assert_eq!(status, 0);
    assert!(String::from_utf8(stdout)
        .unwrap()
        .contains("golden: 0 passed, 0 failed, 0 cancelled"));
    assert!(stderr.is_empty());
    assert!(!artifact_root.exists());
}

#[test]
fn empty_machine_reports_are_valid_single_documents() {
    let fixture = Fixture::new();
    let artifact_root = fixture.root.with_extension("artifacts");
    for format in ["json", "junit"] {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = run_cli_with_context(
            [
                "skald-golden",
                "--filter",
                "absent/**",
                "--allow-empty",
                "--format",
                format,
            ]
            .map(Into::into),
            &fixture.root,
            &artifact_root,
            &fixture.root,
            &mut stdout,
            &mut stderr,
        );
        assert_eq!(status, 0);
        assert!(stderr.is_empty());
        if format == "json" {
            let value: serde_json::Value = serde_json::from_slice(&stdout).unwrap();
            assert_eq!(value["cases"].as_array().unwrap().len(), 0);
        } else {
            let output = String::from_utf8(stdout).unwrap();
            assert!(output.starts_with("<?xml version="));
            assert!(output.ends_with("</testsuite>\n"));
        }
    }
}

struct BrokenWriter;

impl Write for BrokenWriter {
    fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
        Err(io::Error::new(io::ErrorKind::BrokenPipe, "consumer exited"))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn broken_pipes_from_list_help_and_reports_are_successful_and_silent() {
    let fixture = Fixture::new();
    let artifact_root = fixture.root.with_extension("artifacts");
    for arguments in [
        vec!["skald-golden", "--list"],
        vec!["skald-golden", "--help"],
        vec![
            "skald-golden",
            "--filter",
            "absent/**",
            "--allow-empty",
            "--format",
            "json",
        ],
    ] {
        let mut stderr = Vec::new();
        let status = run_cli_with_context(
            arguments.into_iter().map(Into::into),
            &fixture.root,
            &artifact_root,
            &fixture.root,
            &mut BrokenWriter,
            &mut stderr,
        );
        assert_eq!(status, 0);
        assert!(stderr.is_empty());
    }
}
