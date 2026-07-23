use super::*;

#[test]
fn assembly_mode_runs_the_pipeline_and_writes_only_assembly() {
    let directory = TemporaryDirectory::new("driver-assembly").unwrap();
    let input = directory.join("answer.ska");
    let output = directory.join("answer.s");
    fs::write(&input, "fn main() -> i64 { return 42; }").unwrap();
    fs::write(&output, "previous artifact").unwrap();

    let owned = [
        OsString::from("skac"),
        input.clone().into_os_string(),
        OsString::from("--emit"),
        OsString::from("asm"),
        OsString::from("-o"),
        output.clone().into_os_string(),
    ];
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_cli_with_context(
        owned,
        &mut stdout,
        &mut stderr,
        &Toolchain::new("false", "missing-runtime.a"),
    )
    .unwrap();

    assert_eq!(status, 0, "{}", String::from_utf8_lossy(&stderr));
    assert!(stdout.is_empty());
    assert!(stderr.is_empty());
    let text = fs::read_to_string(output).unwrap();
    assert!(text.starts_with(".intel_syntax noprefix\n"));
    assert!(text.contains(".globl main"));
    assert!(text.contains("mov rax, 42"));
    assert!(temporary_artifacts(directory.path()).is_empty());
}

#[test]
fn assembly_mode_uses_the_default_suffixed_output_path() {
    let directory = TemporaryDirectory::new("driver-default-assembly").unwrap();
    let input = directory.join("answer.ska");
    let output = directory.join("answer.s");
    fs::write(&input, "fn main() -> i64 { return 42; }").unwrap();
    let args = [
        OsString::from("skac"),
        input.into_os_string(),
        OsString::from("--emit"),
        OsString::from("asm"),
    ];
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let status = run_cli_with_context(
        args,
        &mut stdout,
        &mut stderr,
        &Toolchain::new("false", "missing-runtime.a"),
    )
    .unwrap();

    assert_eq!(status, 0, "{}", String::from_utf8_lossy(&stderr));
    assert!(stdout.is_empty());
    assert!(stderr.is_empty());
    assert!(fs::read_to_string(output).unwrap().contains(".globl main"));
    assert!(temporary_artifacts(directory.path()).is_empty());
}

#[test]
fn explicit_output_must_not_alias_the_input_source() {
    let directory = TemporaryDirectory::new("driver-input-alias").unwrap();
    let input = directory.join("source.ska");
    let source = "fn main() -> i64 { return 42; }";
    fs::write(&input, source).unwrap();

    let symlink = directory.join("source-symlink.ska");
    std::os::unix::fs::symlink(&input, &symlink).unwrap();
    let hard_link = directory.join("source-hard-link.ska");
    fs::hard_link(&input, &hard_link).unwrap();

    for output in [&input, &symlink, &hard_link] {
        let args = [
            OsString::from("skac"),
            input.clone().into_os_string(),
            OsString::from("--emit"),
            OsString::from("asm"),
            OsString::from("-o"),
            output.clone().into_os_string(),
        ];
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = run_cli_with_context(
            args,
            &mut stdout,
            &mut stderr,
            &Toolchain::new("false", "missing-runtime.a"),
        )
        .unwrap();

        assert_eq!(status, EXIT_USAGE);
        assert!(stdout.is_empty());
        assert!(String::from_utf8(stderr)
            .unwrap()
            .contains("output path must not refer to the input source"));
        assert_eq!(fs::read_to_string(&input).unwrap(), source);
    }

    assert!(temporary_artifacts(directory.path()).is_empty());
}

#[test]
fn compilation_failure_preserves_existing_assembly_output() {
    let directory = TemporaryDirectory::new("driver-compile-failure").unwrap();
    let input = directory.join("broken.ska");
    let output = directory.join("broken.s");
    fs::write(&input, "fn main() -> i64 { return unknown; }").unwrap();
    fs::write(&output, "previous artifact").unwrap();
    let args = [
        OsString::from("skac"),
        input.into_os_string(),
        OsString::from("--emit"),
        OsString::from("asm"),
        OsString::from("-o"),
        output.clone().into_os_string(),
    ];
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let status = run_cli_with_context(
        args,
        &mut stdout,
        &mut stderr,
        &Toolchain::new("false", "missing-runtime.a"),
    )
    .unwrap();

    assert_eq!(status, EXIT_COMPILE_ERROR);
    assert!(stdout.is_empty());
    assert!(String::from_utf8(stderr)
        .unwrap()
        .contains("unknown name `unknown`"));
    assert_eq!(fs::read_to_string(output).unwrap(), "previous artifact");
    assert!(temporary_artifacts(directory.path()).is_empty());
}
