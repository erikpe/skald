use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use crate::{backend::Target, diagnostics::render_diagnostics};

use super::*;

static NEXT_TEST_ID: AtomicU64 = AtomicU64::new(0);

fn run(args: &[&str]) -> (i32, String, String) {
    run_with_toolchain(args, &Toolchain::new("false", "missing-runtime.a"))
}

fn run_with_toolchain(args: &[&str], toolchain: &Toolchain) -> (i32, String, String) {
    let args = args.iter().map(OsString::from);
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let exit_code = run_cli_with_context(args, &mut stdout, &mut stderr, toolchain).unwrap();

    (
        exit_code,
        String::from_utf8(stdout).unwrap(),
        String::from_utf8(stderr).unwrap(),
    )
}

fn test_directory(name: &str) -> PathBuf {
    let id = NEXT_TEST_ID.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "skald-driver-test-{}-{id}-{name}",
        std::process::id()
    ));
    fs::create_dir(&path).unwrap();
    path
}

#[test]
fn help_and_version_are_available_without_compilation() {
    let (exit_code, stdout, stderr) = run(&["skac", "--help"]);
    assert_eq!(exit_code, 0);
    assert_eq!(stdout, format!("{HELP}\n"));
    assert!(stderr.is_empty());

    let (exit_code, stdout, stderr) = run(&["skac", "--version"]);
    assert_eq!(exit_code, 0);
    assert_eq!(stdout, format!("skac {}\n", env!("CARGO_PKG_VERSION")));
    assert!(stderr.is_empty());
}

#[test]
fn invalid_arguments_are_usage_errors() {
    let (exit_code, stdout, stderr) = run(&["skac"]);
    assert_eq!(exit_code, EXIT_USAGE);
    assert!(stdout.is_empty());
    assert!(stderr.starts_with("skac: no input file was provided\n"));

    let (exit_code, _, stderr) = run(&["skac", "test.ska", "--emit", "object"]);
    assert_eq!(exit_code, EXIT_USAGE);
    assert!(stderr.contains("unsupported emission kind `object`; expected `asm`"));
}

#[test]
fn assembly_mode_runs_the_pipeline_and_writes_only_assembly() {
    let directory = test_directory("assembly");
    let input = directory.join("answer.ska");
    let output = directory.join("answer.s");
    fs::write(&input, "fn main() -> i64 { return 42; }").unwrap();

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
    assert!(text.contains(".globl main"));
    assert!(text.contains("movabsq $42, %rax"));
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn source_diagnostics_are_rendered_and_return_compilation_failure() {
    let directory = test_directory("diagnostic");
    let input = directory.join("broken.ska");
    fs::write(&input, "fn main() -> i64 { return nope; }").unwrap();
    let args = [
        OsString::from("skac"),
        input.clone().into_os_string(),
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

    assert_eq!(status, EXIT_COMPILE_ERROR);
    assert!(stdout.is_empty());
    assert!(String::from_utf8(stderr)
        .unwrap()
        .contains("error[RES003]: unknown name `nope`"));
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn linker_failure_is_a_driver_error_not_a_panic() {
    let directory = test_directory("toolchain-failure");
    let input = directory.join("valid.ska");
    let output = directory.join("valid");
    fs::write(&input, "fn main() -> i64 { return 0; }").unwrap();
    let runtime_placeholder = directory.join("runtime.a");
    fs::write(&runtime_placeholder, "placeholder").unwrap();
    let args = [
        OsString::from("skac"),
        input.into_os_string(),
        OsString::from("-o"),
        output.into_os_string(),
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
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn unresolved_source_external_is_reported_as_a_toolchain_failure() {
    let directory = test_directory("unresolved-external");
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
    fs::remove_dir_all(directory).unwrap();
}

#[test]
fn composes_the_complete_frontend_and_backend_pipeline() {
    let artifact = compile_source_to_assembly(
        "complete.ska",
        "fn double(x: i64) -> i64 { return x * 2; }\n\
         fn main() -> i64 { return double(21); }",
        Target::X86_64SysV,
    )
    .unwrap();

    assert!(artifact.report.diagnostics.is_empty());
    assert!(artifact.assembly.contains("call .Lska_fn_0"));
    assert!(artifact.assembly.contains(".globl main"));
}

#[test]
fn stops_before_semantic_phases_after_a_source_error() {
    let CompilationError::Diagnostics(report) = compile_source_to_assembly(
        "broken.ska",
        "fn main() -> i64 { return @; }",
        Target::X86_64SysV,
    )
    .unwrap_err() else {
        panic!("expected source diagnostics");
    };

    let rendered = render_diagnostics(&report.sources, &report.diagnostics);
    assert!(rendered.contains("error[LEX001]: unexpected character `@`"));
    assert!(!rendered.contains("PAR"));
}

#[test]
fn malformed_first_slice_sources_never_panic() {
    let valid = "fn main() -> i64 { var value: i64 = 1; return value + 2; }";
    let mut malformed: Vec<&str> = valid
        .char_indices()
        .map(|(offset, _)| &valid[..offset])
        .collect();
    malformed.extend([
        "@",
        "fn",
        "fn main(",
        "fn main() -> i64 { return ; }",
        "fn main() -> i64 { var x: i64 = ; return 0; }",
        "fn main() -> i64 { (((((((; }",
        "fn main() -> i64 { return 12abc; }",
        "fn main() -> i64 { if 1 { return 0; } }",
        "fn main() -> bool { return 0; }",
        "fn main() -> i64 { return unknown(1, 2); }",
        "extern",
        "extern fn",
        "extern fn missing(",
        "extern fn missing() -> unit fn main() -> i64 { return 0; }",
    ]);

    for (index, source) in malformed.into_iter().enumerate() {
        let result = std::panic::catch_unwind(|| {
            compile_source_to_assembly(format!("malformed-{index}.ska"), source, Target::X86_64SysV)
        });
        assert!(
            result.is_ok(),
            "compiler panicked for malformed input {source:?}"
        );
        assert!(
            matches!(result.unwrap(), Err(CompilationError::Diagnostics(_))),
            "malformed input did not produce source diagnostics: {source:?}"
        );
    }
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
