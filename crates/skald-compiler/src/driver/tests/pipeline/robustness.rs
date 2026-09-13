use std::{ffi::OsString, fs};

use crate::{
    backend::Target,
    diagnostics::render_diagnostics,
    driver::{
        compile_source_to_assembly, run_cli_with_context, CompilationError, Toolchain,
        EXIT_COMPILE_ERROR,
    },
    syntax::{EXCESSIVE_NESTING, MAX_SYNTAX_NESTING},
    test_support::TemporaryDirectory,
};

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
        "class Value { init() {} } extern fn inspect(ref value: Value) -> unit; fn main() -> i64 { return 0; }",
        "class Value { init() {} } fn inspect(mut ref value: Value) -> unit {} fn forward(ref value: Value) -> unit { inspect(value); } fn main() -> i64 { return 0; }",
        "class Value { init() {} } fn inspect(mut ref value: Value) -> unit {} fn main() -> i64 { inspect(Value()); return 0; }",
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
