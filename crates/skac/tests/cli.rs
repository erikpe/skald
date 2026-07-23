mod support;

use std::{fs, process::Command};

use support::TemporaryDirectory;

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
    let directory = TemporaryDirectory::new("assembly").unwrap();
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
    assert!(assembly_text.starts_with(".intel_syntax noprefix\n"));
    assert!(assembly_text.contains("imul rax, rcx"));
    assert!(assembly_text.contains(".globl main"));
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
