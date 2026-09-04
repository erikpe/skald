use std::{path::Path, process::Command};

fn repository_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
}

#[test]
fn independent_processes_emit_identical_structural_json() {
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_skald-mir-measure"))
            .args(["--format", "json", "--workload", "benchmark/range-i64"])
            .current_dir(repository_root())
            .output()
            .unwrap()
    };
    let first = run();
    let second = run();
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(
        second.status.success(),
        "{}",
        String::from_utf8_lossy(&second.stderr)
    );
    assert_eq!(first.stdout, second.stdout);
    let report: serde_json::Value = serde_json::from_slice(&first.stdout).unwrap();
    assert_eq!(report["workloads"][0]["id"], "benchmark/range-i64");
    assert!(report["workloads"][0].get("operational").is_none());
}

#[test]
fn help_is_read_only_and_documents_the_opt_in_surface() {
    let output = Command::new(env!("CARGO_BIN_EXE_skald-mir-measure"))
        .arg("--help")
        .current_dir(repository_root())
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("--workload ID"));
    assert!(stdout.contains("--operational"));
}
