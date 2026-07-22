use super::*;

#[test]
fn linker_failure_is_a_driver_error_not_a_panic() {
    let directory = TemporaryDirectory::new("driver-toolchain-failure").unwrap();
    let input = directory.join("valid.ska");
    let output = directory.join("valid");
    fs::write(&input, "fn main() -> i64 { return 0; }").unwrap();
    fs::write(&output, "previous executable").unwrap();
    let runtime_placeholder = directory.join("runtime.a");
    fs::write(&runtime_placeholder, "placeholder").unwrap();
    let args = [
        OsString::from("skac"),
        input.into_os_string(),
        OsString::from("-o"),
        output.clone().into_os_string(),
    ];
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let status = run_cli_with_context(
        args,
        &mut stdout,
        &mut stderr,
        &Toolchain::new("false", runtime_placeholder),
    )
    .unwrap();

    assert_eq!(status, EXIT_COMPILE_ERROR);
    assert!(stdout.is_empty());
    assert_eq!(
        String::from_utf8(stderr).unwrap(),
        "skac: toolchain `false` failed with exit status 1\n"
    );
    assert_eq!(fs::read_to_string(output).unwrap(), "previous executable");
    assert!(temporary_artifacts(directory.path()).is_empty());
}

#[test]
fn unresolved_source_external_is_reported_as_a_toolchain_failure() {
    let directory = TemporaryDirectory::new("driver-unresolved-external").unwrap();
    let input = directory.join("unresolved.ska");
    let output = directory.join("unresolved");
    fs::write(
        &input,
        concat!(
            "extern fn definitely_missing_skald_test_symbol() -> unit;\n",
            "fn main() -> i64 { definitely_missing_skald_test_symbol(); return 0; }\n",
        ),
    )
    .unwrap();
    let empty_archive = directory.join("empty-runtime.a");
    fs::write(&empty_archive, b"!<arch>\n").unwrap();
    let args = [
        OsString::from("skac"),
        input.into_os_string(),
        OsString::from("-o"),
        output.clone().into_os_string(),
    ];
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    let status = run_cli_with_context(
        args,
        &mut stdout,
        &mut stderr,
        &Toolchain::new("cc", empty_archive),
    )
    .unwrap();

    assert_eq!(status, EXIT_COMPILE_ERROR);
    assert!(stdout.is_empty());
    assert!(String::from_utf8(stderr)
        .unwrap()
        .contains("skac: toolchain `cc` failed with exit status"));
    assert!(!output.exists());
}

#[test]
fn runtime_archive_without_current_abi_marker_is_a_toolchain_failure() {
    let directory = TemporaryDirectory::new("driver-runtime-abi-mismatch").unwrap();
    let output = directory.join("program");
    fs::write(&output, "previous executable").unwrap();
    let incompatible_archive = directory.join("libskald_runtime.a");
    fs::write(&incompatible_archive, b"!<arch>\n").unwrap();
    let assembly = compile_source_to_assembly(
        "compatible-source.ska",
        "fn main() -> i64 { return 0; }",
        Target::X86_64SysV,
    )
    .unwrap()
    .assembly;

    let error = Toolchain::new("cc", incompatible_archive)
        .link_assembly(&assembly, &output)
        .unwrap_err();

    let ToolchainError::Failed {
        tool,
        exit_code,
        details,
    } = error
    else {
        panic!("expected a structured linker failure, got {error:?}");
    };
    assert_eq!(tool, OsString::from("cc"));
    assert!(exit_code.is_some());
    assert!(
        details.contains("ska_rt_abi_v4"),
        "linker did not identify the missing ABI marker: {details}"
    );
    assert_eq!(fs::read_to_string(output).unwrap(), "previous executable");
    assert!(temporary_artifacts(directory.path()).is_empty());
}

#[test]
fn missing_runtime_is_reported_before_spawning_the_toolchain() {
    let toolchain = Toolchain::new("does-not-exist", "does-not-exist.a");
    let error = toolchain
        .link_assembly("", Path::new("unused-output"))
        .unwrap_err();

    assert_eq!(
        error.to_string(),
        "Skald runtime archive is unavailable; run `make runtime` or set SKALD_RUNTIME_ARCHIVE"
    );
}

#[test]
fn subprocess_failure_includes_the_tool_and_status() {
    let runtime_placeholder = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let toolchain = Toolchain::new("false", runtime_placeholder);
    let error = toolchain
        .link_assembly("", Path::new("unused-output"))
        .unwrap_err();

    assert_eq!(
        error.to_string(),
        "toolchain `false` failed with exit status 1"
    );
}

#[test]
fn process_start_failure_is_structured() {
    let runtime_placeholder = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let toolchain = Toolchain::new("skald-test-tool-that-does-not-exist", runtime_placeholder);
    let error = toolchain
        .link_assembly("", Path::new("unused-output"))
        .unwrap_err();

    assert!(error
        .to_string()
        .starts_with("could not start toolchain `skald-test-tool-that-does-not-exist`:"));
}
