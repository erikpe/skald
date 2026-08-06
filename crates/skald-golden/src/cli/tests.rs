use super::run_cli_with_context;
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
fn read_only_cli_operations_render_the_validated_plan() {
    let fixture = Fixture::new();
    let artifact_root = fixture.root.with_extension("artifacts");
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_cli_with_context(
        ["skald-golden", "--list"].map(Into::into),
        &fixture.root,
        &artifact_root,
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
fn execution_remains_unavailable() {
    let fixture = Fixture::new();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_cli_with_context(
        ["skald-golden"].map(Into::into),
        &fixture.root,
        &fixture.root.with_extension("artifacts"),
        &mut stdout,
        &mut stderr,
    );
    assert_eq!(status, 2);
    assert!(stdout.is_empty());
    assert!(String::from_utf8(stderr)
        .unwrap()
        .contains("not implemented"));
}
