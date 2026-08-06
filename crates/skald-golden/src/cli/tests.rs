use super::{options::Options, run_cli_with_context};
use crate::Determinism;
use std::{
    fs,
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
        ]
        .map(Into::into),
    )
    .unwrap();

    assert_eq!(options.compiler, Some(PathBuf::from("tools/skac")));
    assert_eq!(options.determinism, Determinism::Full);
    assert_eq!(options.jobs.unwrap().get(), 3);
    assert!(options.fail_fast);
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
    assert_eq!(stdout, b"golden: no selected leaves\n");
    assert!(stderr.is_empty());
    assert!(!artifact_root.exists());
}
