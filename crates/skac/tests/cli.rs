use std::{
    fs,
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT_TEST_ID: AtomicU64 = AtomicU64::new(0);

fn test_directory(name: &str) -> PathBuf {
    let id = NEXT_TEST_ID.fetch_add(1, Ordering::Relaxed);
    let path =
        std::env::temp_dir().join(format!("skac-cli-test-{}-{id}-{name}", std::process::id()));
    fs::create_dir(&path).unwrap();
    path
}

#[test]
fn help_succeeds_through_the_binary_entry_point() {
    let output = Command::new(env!("CARGO_BIN_EXE_skac"))
        .arg("--help")
        .output()
        .expect("failed to execute skac");

    assert!(output.status.success());
    assert!(String::from_utf8(output.stdout)
        .expect("skac stdout was not UTF-8")
        .starts_with("skac - the Skald compiler\n"));
    assert!(output.stderr.is_empty());
}

#[test]
fn assembly_output_runs_the_real_pipeline_through_the_binary() {
    let directory = test_directory("assembly");
    let source = directory.join("answer.ska");
    let assembly = directory.join("answer.s");
    fs::write(&source, "fn main() -> i64 { return 6 * 7; }\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_skac"))
        .arg(&source)
        .args(["--emit", "asm", "-o"])
        .arg(&assembly)
        .output()
        .expect("failed to execute skac");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    let assembly_text = fs::read_to_string(&assembly).unwrap();
    assert!(assembly_text.contains("imulq %rcx, %rax"));
    assert!(assembly_text.contains(".globl main"));
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn missing_source_is_reported_as_an_io_error() {
    let output = Command::new(env!("CARGO_BIN_EXE_skac"))
        .arg("missing.ska")
        .output()
        .expect("failed to execute skac");

    assert_eq!(output.status.code(), Some(74));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8(output.stderr)
        .expect("skac stderr was not UTF-8")
        .starts_with("skac: could not read `missing.ska`:"));
}
