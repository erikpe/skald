use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
};

use crate::{
    backend::Target,
    diagnostics::render_diagnostics,
    syntax::{EXCESSIVE_NESTING, MAX_SYNTAX_NESTING},
    test_support::TemporaryDirectory,
};

use super::*;

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

fn temporary_artifacts(directory: &Path) -> Vec<PathBuf> {
    fs::read_dir(directory)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.contains(".skac-") && name.ends_with(".tmp"))
        })
        .collect()
}

#[test]
fn unused_destructor_bodies_lower_through_the_backend() {
    let artifact = compile_source_to_assembly(
        "destructor.ska",
        concat!(
            "class Resource { value: i64; init() { self.value = 0; } destroy { self.value = 1; } }\n",
            "fn main() -> i64 { return 0; }\n",
        ),
        Target::X86_64SysV,
    )
    .expect("DD5 must lower valid destructor definitions deterministically");

    assert!(artifact.report.diagnostics.is_empty());
    assert!(artifact.assembly.contains(".Lska_class_0_destroy_0"));
}

#[test]
fn unused_copy_lifecycle_bodies_lower_to_mir_member_definitions() {
    let artifact = compile_source_to_assembly(
        "copy-lifecycle.ska",
        concat!(
            "class Value {\n",
            "  value: i64;\n",
            "  init(value: i64) { self.value = value; }\n",
            "  init(ref other: Value) { self.value = other.value; }\n",
            "  assign(ref other: Value) { self.value = other.value; }\n",
            "}\n",
            "fn main() -> i64 { return 0; }\n",
        ),
        Target::X86_64SysV,
    )
    .expect("OVS4 copy lifecycle bodies must lower as MIR member definitions");

    assert!(artifact.report.diagnostics.is_empty());
    assert!(artifact.assembly.contains(".Lska_class_0_init_0"));
    assert!(artifact.assembly.contains(".Lska_class_0_init_1"));
    assert!(artifact.assembly.contains(".Lska_class_0_assign_0"));
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
    assert!(text.contains(".globl main"));
    assert!(text.contains("movabsq $42, %rax"));
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

#[test]
fn source_diagnostics_are_rendered_and_return_compilation_failure() {
    let directory = TemporaryDirectory::new("driver-diagnostic").unwrap();
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
}

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
fn composes_the_complete_object_frontend_and_backend_pipeline() {
    let artifact = compile_source_to_assembly(
        "object.ska",
        concat!(
            "class Box { value: i64; init(value: i64) { self.value = value; } ",
            "fn get() -> i64 { return self.value; } } ",
            "fn main() -> i64 { var value: Box = Box(42); return value.get(); }",
        ),
        Target::X86_64SysV,
    )
    .unwrap();

    assert!(artifact.report.diagnostics.is_empty());
    assert!(artifact.assembly.contains("call .Lska_class_0_init_0"));
    assert!(artifact.assembly.contains("call .Lska_class_0_method_0"));
}

#[test]
fn ovs5_copy_source_reaches_the_backend() {
    let artifact = compile_source_to_assembly(
        "copy.ska",
        concat!(
            "class Value { init() {} }\n",
            "fn main() -> i64 {\n",
            "  var source: Value = Value();\n",
            "  var copy: Value = source;\n",
            "  copy = source;\n",
            "  return 0;\n",
            "}\n",
        ),
        Target::X86_64SysV,
    )
    .expect("OVS5 local copy operations must lower through the backend");
    assert!(artifact.report.diagnostics.is_empty());
    assert!(!artifact.assembly.contains("memcpy"));
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
fn typed_alias_syntax_reaches_the_backend_pipeline() {
    let artifact = compile_source_to_assembly(
        "alias-syntax.ska",
        concat!(
            "class Dog { init() {} }\n",
            "fn inspect(ref dog: Dog) -> unit {}\n",
            "fn main() -> i64 { return 0; }\n",
        ),
        Target::X86_64SysV,
    )
    .unwrap();

    assert!(artifact.report.diagnostics.is_empty());
    assert!(artifact.assembly.contains(".Lska_fn_0:"));
}

#[test]
fn malformed_supported_sources_never_panic() {
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
        "class Broken { init() {} destroy(",
        "class Broken { init() {} destroy -> unit {} } fn main() -> i64 { return 0; }",
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
fn malformed_and_excluded_destructor_sources_fail_before_backend_lowering() {
    let cases = [
        "class Resource { init() {} destroy {} destroy {} } fn main() -> i64 { return 0; }",
        "class Resource { init() {} destroy { return 1; } } fn main() -> i64 { return 0; }",
        "class Resource { init() {} destroy { self.destroy(); } } fn main() -> i64 { return 0; }",
        "class Leaf { init() {} } class Owner { leaf: Leaf; init() { self.leaf = Leaf(); } destroy { self.leaf = Leaf(); } } fn main() -> i64 { return 0; }",
    ];

    for (index, source) in cases.into_iter().enumerate() {
        let result = std::panic::catch_unwind(|| {
            compile_source_to_assembly(
                format!("malformed-destructor-{index}.ska"),
                source,
                Target::X86_64SysV,
            )
        });
        assert!(
            result.is_ok(),
            "compiler panicked for malformed destructor case {index}"
        );
        assert!(
            matches!(result.unwrap(), Err(CompilationError::Diagnostics(_))),
            "malformed destructor case {index} crossed the diagnostic boundary"
        );
    }
}

#[test]
fn malformed_and_excluded_alias_sources_never_reach_mir_or_backend_panics() {
    let cases = [
        "class Value { init() {} } fn malformed(ref mut value: Value) -> unit {} fn main() -> i64 { return 0; }",
        "fn inspect(ref value: i64) -> unit {} fn main() -> i64 { return 0; }",
        "class Value { init() {} } extern fn inspect(ref value: Value) -> unit; fn main() -> i64 { return 0; }",
        "class Value { init() {} } fn inspect(mut ref value: Value) -> unit {} fn forward(ref value: Value) -> unit { inspect(value); } fn main() -> i64 { return 0; }",
        "class Value { init() {} } fn inspect(ref value: Value) -> unit {} fn main() -> i64 { inspect(Value()); return 0; }",
    ];

    for (index, source) in cases.into_iter().enumerate() {
        let result = compile_source_to_assembly(
            format!("malformed-alias-{index}.ska"),
            source,
            Target::X86_64SysV,
        );
        assert!(
            matches!(result, Err(CompilationError::Diagnostics(_))),
            "malformed alias case {index} crossed the diagnostic boundary: {result:?}"
        );
    }
}

#[test]
fn malformed_and_excluded_inline_field_sources_fail_before_backend_lowering() {
    let cases = [
        (
            "unknown-class-field",
            "class Root { child: Missing; init() {} } fn main() -> i64 { return 0; }",
        ),
        (
            "recursive-containment",
            concat!(
                "class Root { child: Root; init() { self.child = Root(); } } ",
                "fn main() -> i64 { return 0; }",
            ),
        ),
        (
            "grouped-construction",
            concat!(
                "class Child { init() {} } ",
                "class Root { child: Child; init() { self.child = (Child()); } } ",
                "fn main() -> i64 { return 0; }",
            ),
        ),
        (
            "premature-projection",
            concat!(
                "class Child { value: i64; init() { self.value = 0; } ",
                "fn get() -> i64 { return self.value; } } ",
                "class Root { child: Child; value: i64; init() { ",
                "self.value = self.child.get(); self.child = Child(); } } ",
                "fn main() -> i64 { return 0; }",
            ),
        ),
        (
            "object-value",
            concat!(
                "class Child { init() {} } ",
                "class Root { child: Child; init() { self.child = Child(); } } ",
                "fn copy(ref root: Root) -> i64 { return root.child; } ",
                "fn main() -> i64 { return 0; }",
            ),
        ),
        (
            "readonly-projection",
            concat!(
                "class Child { value: i64; init() { self.value = 0; } } ",
                "class Root { child: Child; init() { self.child = Child(); } } ",
                "fn write(ref root: Root) -> unit { root.child.value = 1; } ",
                "fn main() -> i64 { return 0; }",
            ),
        ),
    ];

    for (case, source) in cases {
        let result = std::panic::catch_unwind(|| {
            compile_source_to_assembly(
                format!("malformed-inline-field-{case}.ska"),
                source,
                Target::X86_64SysV,
            )
        });
        let compilation = result
            .unwrap_or_else(|_| panic!("compiler panicked for malformed inline-field case {case}"));
        assert!(
            matches!(compilation, Err(CompilationError::Diagnostics(_))),
            "malformed inline-field case {case} crossed the diagnostic boundary: {compilation:?}"
        );
    }
}

#[test]
fn excessive_syntax_nesting_is_a_source_error_not_a_panic() {
    let expression = format!(
        "{}1{}",
        "(".repeat(MAX_SYNTAX_NESTING),
        ")".repeat(MAX_SYNTAX_NESTING)
    );
    let source = format!("fn main() -> i64 {{ return {expression}; }}");

    let result = std::panic::catch_unwind(|| {
        compile_source_to_assembly("too-deep.ska", source, Target::X86_64SysV)
    });
    let CompilationError::Diagnostics(report) = result
        .expect("excessive syntax nesting must not panic")
        .expect_err("excessive syntax nesting must fail compilation")
    else {
        panic!("expected source diagnostics");
    };

    assert_eq!(report.diagnostics.len(), 1);
    assert_eq!(
        report.diagnostics.iter().next().unwrap().code,
        EXCESSIVE_NESTING
    );
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
