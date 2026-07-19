use std::process::Command;

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
fn compilation_is_an_explicit_usage_error_until_the_pipeline_is_connected() {
    let output = Command::new(env!("CARGO_BIN_EXE_skac"))
        .arg("input.ska")
        .output()
        .expect("failed to execute skac");

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).expect("skac stderr was not UTF-8"),
        "skac: the first vertical compiler slice is not implemented yet\n"
    );
}
