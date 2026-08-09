use super::*;

fn write_canonical_standard_library(root: &Path) {
    for (relative, source) in canonical_standard_library_sources(&[]) {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, source).unwrap();
    }
}

fn module_request(
    directory: &TemporaryDirectory,
    entry: EntrySelector,
    roots: Vec<PathBuf>,
) -> CompilationRequest {
    CompilationRequest::new(
        entry,
        roots,
        StandardLibrarySelection::Disabled,
        Target::X86_64SysV,
        ArtifactOptions::new(ArtifactKind::Assembly, None),
        CompilationEnvironment::new(directory.path().to_owned(), directory.join("unused-std")),
    )
}

#[test]
fn request_pipeline_compiles_the_reachable_multi_module_program() {
    let directory = TemporaryDirectory::new("request-pipeline").unwrap();
    let root = directory.join("modules");
    fs::create_dir_all(root.join("app")).unwrap();
    fs::create_dir_all(root.join("lib")).unwrap();
    fs::write(
        root.join("app/main.ska"),
        concat!(
            "import lib::answer;\n",
            "fn main() -> i64 { return lib::answer::value(); }\n",
        ),
    )
    .unwrap();
    fs::write(
        root.join("lib/answer.ska"),
        "public fn value() -> i64 { return 42; }\n",
    )
    .unwrap();
    let request = module_request(
        &directory,
        EntrySelector::Module("app::main".parse().unwrap()),
        vec![root],
    );

    let artifact = compile_request_to_assembly(&request).unwrap();

    assert!(artifact.report.diagnostics.is_empty());
    assert_eq!(artifact.report.sources.len(), 2);
    assert!(artifact
        .assembly
        .contains("call .Lska.fn.lib.answer.value.f1"));
    assert!(artifact.assembly.contains(".globl main"));
}

#[test]
fn request_pipeline_ignores_malformed_sources_outside_the_reachable_closure() {
    let directory = TemporaryDirectory::new("request-reachability").unwrap();
    let root = directory.join("modules");
    fs::create_dir_all(root.join("app")).unwrap();
    fs::create_dir_all(root.join("unused")).unwrap();
    fs::write(
        root.join("app/main.ska"),
        "fn main() -> i64 { return 42; }\n",
    )
    .unwrap();
    fs::write(root.join("unused/malformed.ska"), "fn broken( {\n").unwrap();
    let request = module_request(
        &directory,
        EntrySelector::Module("app::main".parse().unwrap()),
        vec![root],
    );

    let artifact = compile_request_to_assembly(&request).unwrap();

    assert!(artifact.report.diagnostics.is_empty());
    assert_eq!(artifact.report.sources.len(), 1);
    assert!(artifact.assembly.contains("mov rax, 42"));
}

#[test]
fn literal_program_reaches_target_emission() {
    let directory = TemporaryDirectory::new("request-string-target-emission").unwrap();
    let root = directory.join("modules");
    fs::create_dir_all(root.join("std")).unwrap();
    fs::write(
        root.join("app.ska"),
        concat!(
            "from std::str import Str;\n",
            "fn main() -> i64 { var value: Str = \"typed\"; return 0; }\n",
        ),
    )
    .unwrap();
    fs::write(
        root.join("std/str.ska"),
        concat!(
            "public class Str {\n",
            "  private _storage: shared u8[];\n",
            "  private _start: i64;\n",
            "  private _length: u64;\n",
            "  init() { self._storage = new u8[](); self._start = 0; self._length = 0u; }\n",
            "}\n",
        ),
    )
    .unwrap();
    let request = module_request(
        &directory,
        EntrySelector::Module("app".parse().unwrap()),
        vec![root],
    );

    let artifact = compile_request_to_assembly(&request).unwrap();
    assert!(artifact.report.diagnostics.is_empty());
    assert!(artifact.assembly.contains(".Lska_literal_0_backing:"));
    assert!(artifact.assembly.contains(".quad 0xffffffffffffffff"));
}

#[test]
fn request_pipeline_accepts_a_positional_entry_outside_all_roots() {
    let directory = TemporaryDirectory::new("request-singleton").unwrap();
    let spaced_directory = directory.join("directory with spaces");
    fs::create_dir(&spaced_directory).unwrap();
    let input = spaced_directory.join("outside_main.ska");
    fs::write(&input, "fn main() -> i64 { return 42; }\n").unwrap();
    let request = module_request(&directory, EntrySelector::File(input), Vec::new());

    let artifact = compile_request_to_assembly(&request).unwrap();

    assert!(artifact.report.diagnostics.is_empty());
    assert_eq!(artifact.report.sources.len(), 1);
    assert!(artifact.assembly.contains("mov rax, 42"));
}

#[test]
fn replacement_standard_library_validates_the_canonical_panic_intrinsic() {
    let directory = TemporaryDirectory::new("request-panic-intrinsic").unwrap();
    let root = directory.join("modules");
    let standard_library = directory.join("replacement-std");
    fs::create_dir_all(&root).unwrap();
    write_canonical_standard_library(&standard_library);
    fs::write(
        root.join("app.ska"),
        "import std::error;\nfn main() -> i64 { return 0; }\n",
    )
    .unwrap();
    let request = CompilationRequest::new(
        EntrySelector::Module("app".parse().unwrap()),
        vec![root],
        StandardLibrarySelection::Replacement(standard_library.clone()),
        Target::X86_64SysV,
        ArtifactOptions::new(ArtifactKind::Assembly, None),
        CompilationEnvironment::new(directory.path().to_owned(), directory.join("unused-std")),
    );

    let artifact = compile_request_to_assembly(&request).unwrap();
    assert!(artifact.report.diagnostics.is_empty());

    fs::write(
        standard_library.join("std/error.ska"),
        "public fn panic(message: i64) -> unit {}\n",
    )
    .unwrap();
    let CompilationError::Diagnostics(report) = compile_request_to_assembly(&request).unwrap_err()
    else {
        panic!("expected canonical intrinsic diagnostics");
    };
    assert!(report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == crate::resolve::INVALID_INTRINSIC_DECLARATION));
}

#[test]
fn canonical_standard_library_cycle_obeys_default_replacement_and_disabled_selection() {
    let directory = TemporaryDirectory::new("request-standard-library-cycle").unwrap();
    let application = directory.join("application");
    let installed = directory.join("installed");
    let replacement = directory.join("replacement");
    let self_contained = directory.join("self-contained");
    fs::create_dir_all(&application).unwrap();
    fs::write(
        application.join("app.ska"),
        "from std::str import Str;\nfn main() -> i64 { var value: Str = \"ok\"; return (i64) value.len(); }\n",
    )
    .unwrap();
    write_canonical_standard_library(&installed);
    write_canonical_standard_library(&replacement);
    fs::create_dir_all(&self_contained).unwrap();
    fs::write(
        self_contained.join("app.ska"),
        "from std::str import Str;\nfn main() -> i64 { var value: Str = \"ok\"; return (i64) value.len(); }\n",
    )
    .unwrap();
    write_canonical_standard_library(&self_contained);

    let compile = |roots, standard_library, installed_root| {
        let request = CompilationRequest::new(
            EntrySelector::Module("app".parse().unwrap()),
            roots,
            standard_library,
            Target::X86_64SysV,
            ArtifactOptions::new(ArtifactKind::Assembly, None),
            CompilationEnvironment::new(directory.path().to_owned(), installed_root),
        );
        compile_request_to_assembly(&request)
    };

    for artifact in [
        compile(
            vec![application.clone()],
            StandardLibrarySelection::Default,
            installed.clone(),
        )
        .unwrap(),
        compile(
            vec![application.clone()],
            StandardLibrarySelection::Replacement(replacement),
            directory.join("unused-installed"),
        )
        .unwrap(),
        compile(
            vec![self_contained],
            StandardLibrarySelection::Disabled,
            directory.join("unused-installed"),
        )
        .unwrap(),
    ] {
        assert!(artifact.report.diagnostics.is_empty());
        assert_eq!(artifact.report.sources.len(), 8);
        assert!(artifact.assembly.contains("call ska_rt_panic"));
    }

    let CompilationError::Diagnostics(report) = compile(
        vec![application],
        StandardLibrarySelection::Disabled,
        directory.join("unused-installed"),
    )
    .unwrap_err() else {
        panic!("disabled lookup without a provider-owned standard library must fail");
    };
    assert!(render_diagnostics(&report.sources, &report.diagnostics)
        .contains("module `std::str` was not found"));
}

#[test]
fn canonical_io_obeys_default_replacement_and_disabled_selection() {
    let directory = TemporaryDirectory::new("request-standard-io-providers").unwrap();
    let application = directory.join("application");
    let installed = directory.join("installed");
    let replacement = directory.join("replacement");
    let self_contained = directory.join("self-contained");
    let app_source = concat!(
        "import std::io;\n",
        "from std::str import Str;\n",
        "fn main() -> i64 {\n",
        "  var path: Str = \"input.bin\";\n",
        "  var stdin: Str = std::io::read_stdin();\n",
        "  var file: Str = std::io::read_file(path);\n",
        "  std::io::write_stdout(stdin);\n",
        "  std::io::write_stderr(file);\n",
        "  return 0;\n",
        "}\n",
    );
    fs::create_dir_all(&application).unwrap();
    fs::write(application.join("app.ska"), app_source).unwrap();
    write_canonical_standard_library(&installed);
    write_canonical_standard_library(&replacement);
    fs::create_dir_all(&self_contained).unwrap();
    fs::write(self_contained.join("app.ska"), app_source).unwrap();
    write_canonical_standard_library(&self_contained);

    let compile = |roots, standard_library, installed_root| {
        let request = CompilationRequest::new(
            EntrySelector::Module("app".parse().unwrap()),
            roots,
            standard_library,
            Target::X86_64SysV,
            ArtifactOptions::new(ArtifactKind::Assembly, None),
            CompilationEnvironment::new(directory.path().to_owned(), installed_root),
        );
        compile_request_to_assembly(&request)
    };

    for artifact in [
        compile(
            vec![application.clone()],
            StandardLibrarySelection::Default,
            installed.clone(),
        )
        .unwrap(),
        compile(
            vec![application.clone()],
            StandardLibrarySelection::Replacement(replacement),
            directory.join("unused-installed"),
        )
        .unwrap(),
        compile(
            vec![self_contained],
            StandardLibrarySelection::Disabled,
            directory.join("unused-installed"),
        )
        .unwrap(),
    ] {
        assert!(artifact.report.diagnostics.is_empty());
        assert_eq!(artifact.report.sources.len(), 9);
        for runtime_symbol in [
            "ska_rt_io_standard_handle",
            "ska_rt_io_open",
            "ska_rt_io_read",
            "ska_rt_io_write",
            "ska_rt_io_close",
        ] {
            assert!(artifact
                .assembly
                .contains(&format!("call {runtime_symbol}")));
        }
    }

    let CompilationError::Diagnostics(report) = compile(
        vec![application],
        StandardLibrarySelection::Disabled,
        directory.join("unused-installed"),
    )
    .unwrap_err() else {
        panic!("disabled lookup without a provider-owned standard library must fail");
    };
    assert!(render_diagnostics(&report.sources, &report.diagnostics)
        .contains("module `std::io` was not found"));
}

#[test]
fn installed_process_arguments_reach_verified_assembly_as_ordinary_library_source() {
    let directory = TemporaryDirectory::new("request-process-arguments").unwrap();
    let application = directory.join("application");
    let installed = directory.join("installed");
    fs::create_dir_all(&application).unwrap();
    fs::write(
        application.join("app.ska"),
        concat!(
            "from std::process import args;\n",
            "import std::str;\n",
            "fn main() -> i64 {\n",
            "  var values: std::str::Str[] = args();\n",
            "  return (i64) values.len();\n",
            "}\n",
        ),
    )
    .unwrap();
    write_canonical_standard_library(&installed);
    let request = CompilationRequest::new(
        EntrySelector::Module("app".parse().unwrap()),
        vec![application],
        StandardLibrarySelection::Default,
        Target::X86_64SysV,
        ArtifactOptions::new(ArtifactKind::Assembly, None),
        CompilationEnvironment::new(directory.path().to_owned(), installed),
    );

    let artifact = compile_request_to_assembly(&request).unwrap();

    assert!(artifact.report.diagnostics.is_empty());
    assert_eq!(artifact.report.sources.len(), 10);
    assert!(artifact.assembly.contains(".Lska.fn.std.process.args."));
    assert!(artifact
        .assembly
        .contains("call .Lska.fn.std.io.read_file."));
    assert_eq!(artifact.assembly.matches("call ska_rt_abi_v8\n").count(), 1);
    assert!(artifact.assembly.contains(concat!(
        "main:\n",
        "    push rbp\n",
        "    mov rbp, rsp\n",
        "    call ska_rt_abi_v8\n",
        "    call .Lska.fn.app.main.",
    )));
    for runtime_symbol in [
        "ska_rt_io_standard_handle",
        "ska_rt_io_open",
        "ska_rt_io_read",
        "ska_rt_io_write",
        "ska_rt_io_close",
    ] {
        assert!(artifact
            .assembly
            .contains(&format!("call {runtime_symbol}")));
    }
}

#[test]
fn request_pipeline_preserves_configuration_and_source_failure_categories() {
    let directory = TemporaryDirectory::new("request-failures").unwrap();
    let invalid_root = directory.join("missing-root");
    let request = module_request(
        &directory,
        EntrySelector::Module("app::main".parse().unwrap()),
        vec![invalid_root],
    );
    let CompilationError::ProviderConfiguration(errors) =
        compile_request_to_assembly(&request).unwrap_err()
    else {
        panic!("expected provider configuration failure");
    };
    assert_eq!(errors.len(), 1);

    let missing = module_request(
        &directory,
        EntrySelector::File(directory.join("missing.ska")),
        Vec::new(),
    );
    let CompilationError::Diagnostics(report) = compile_request_to_assembly(&missing).unwrap_err()
    else {
        panic!("expected source diagnostics");
    };
    assert!(render_diagnostics(&report.sources, &report.diagnostics)
        .contains("error[MOD001]: invalid entry"));
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
    .expect("valid destructor definitions must lower deterministically");

    assert!(artifact.report.diagnostics.is_empty());
    assert!(artifact
        .assembly
        .contains(".Lska.class.main.Resource.c0.destroy.d0"));
}

#[test]
fn unused_copy_lifecycle_bodies_lower_to_mir_member_definitions() {
    let artifact = compile_source_to_assembly(
        "copy-lifecycle.ska",
        concat!(
            "class Value {\n",
            "  value: i64;\n",
            "  init(value: i64) { self.value = value; }\n",
            "  copy(ref other: Value) { self.value = other.value; }\n",
            "  assign(ref other: Value) { self.value = other.value; }\n",
            "}\n",
            "fn main() -> i64 { return 0; }\n",
        ),
        Target::X86_64SysV,
    )
    .expect("copy lifecycle bodies must lower as MIR member definitions");

    assert!(artifact.report.diagnostics.is_empty());
    assert!(artifact
        .assembly
        .contains(".Lska.class.main.Value.c0.init.i0"));
    assert!(artifact
        .assembly
        .contains(".Lska.class.main.Value.c0.copy.k0"));
    assert!(artifact
        .assembly
        .contains(".Lska.class.main.Value.c0.assign.a0"));
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
fn composes_the_complete_frontend_and_backend_pipeline() {
    let artifact = compile_source_to_assembly(
        "complete.ska",
        "fn double(x: i64) -> i64 { return x * 2; }\n\
         fn main() -> i64 { return double(21); }",
        Target::X86_64SysV,
    )
    .unwrap();

    assert!(artifact.report.diagnostics.is_empty());
    assert!(artifact.assembly.contains("call .Lska.fn.main.double.f0"));
    assert!(artifact.assembly.contains(".globl main"));
}

#[test]
fn static_inheritance_composes_through_the_complete_pipeline() {
    let artifact = compile_source_to_assembly(
        "inheritance.ska",
        concat!(
            "class Base { value: i64; init(value: i64) { self.value = value; } }\n",
            "class Derived extends Base { init(value: i64) { super(value); } }\n",
            "fn main() -> i64 { var value: Derived = Derived(7); return value.value; }\n",
        ),
        Target::X86_64SysV,
    )
    .unwrap();

    assert!(artifact.report.diagnostics.is_empty());
    assert!(artifact
        .assembly
        .contains("call .Lska.class.main.Base.c0.init.i0"));
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
    assert!(artifact
        .assembly
        .contains("call .Lska.class.main.Box.c0.init.i0"));
    assert!(artifact
        .assembly
        .contains("call .Lska.class.main.Box.c0.method.get.m0"));
}

#[test]
fn local_copy_operations_reach_the_backend() {
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
    .expect("local copy operations must lower through the backend");
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
    assert!(artifact.assembly.contains(".Lska.fn.main.inspect.f0:"));
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

#[test]
fn primitive_inline_array_locals_cross_the_complete_driver_pipeline() {
    let artifact = compile_source_to_assembly(
        "arrays.ska",
        concat!(
            "fn duplicate(values: i64[]) -> i64[] { return values; }\n",
            "fn main() -> i64 {\n",
            "  var values: i64[] = i64[](4u);\n",
            "  values[-1] = 7;\n",
            "  var copied: i64[] = duplicate(values);\n",
            "  return copied[3];\n",
            "}\n",
        ),
        Target::X86_64SysV,
    )
    .expect("primitive inline local arrays must lower through x86-64");
    assert!(artifact.assembly.contains("call ska_rt_alloc"));
    assert!(artifact.assembly.contains("call ska_rt_free"));
    assert!(artifact.assembly.contains(".Lska_array_0_copy_element"));
    assert!(artifact.assembly.contains("[r11 + r10*8 + 16]"));
}

#[test]
fn primitive_array_element_lists_cross_the_complete_driver_pipeline() {
    let artifact = compile_source_to_assembly(
        "array-element-list.ska",
        concat!(
            "fn main() -> i64 {\n",
            "  var values: i64[] = i64[]{1, 2};\n",
            "  return values[0] + values[1];\n",
            "}\n",
        ),
        Target::X86_64SysV,
    )
    .expect("primitive element lists must lower through x86-64");
    assert!(artifact.assembly.contains("call ska_rt_alloc"));
    assert!(artifact.assembly.contains("[r11 + r10*8 + 16]"));
}

#[test]
fn exact_class_array_element_lists_cross_the_complete_driver_pipeline() {
    let artifact = compile_source_to_assembly(
        "array-element-list.ska",
        concat!(
            "class Item { init() {} }\n",
            "fn main() -> i64 {\n",
            "  var values: Item[] = Item[]{Item()};\n",
            "  return 0;\n",
            "}\n",
        ),
        Target::X86_64SysV,
    )
    .expect("exact-class element lists must lower through x86-64");
    assert!(artifact
        .assembly
        .contains("call .Lska.class.main.Item.c0.init.i0"));
}

#[test]
fn inline_optional_array_element_lists_cross_the_complete_driver_pipeline() {
    let artifact = compile_source_to_assembly(
        "array-element-list.ska",
        concat!(
            "class Item { value: i64; init(value: i64) { self.value = value; } }\n",
            "fn main() -> i64 {\n",
            "  var scalars: i64?[] = i64?[]{none, 2};\n",
            "  var objects: shared Item?[] = new Item?[]{none, Item(3)};\n",
            "  return scalars[1]!;\n",
            "}\n",
        ),
        Target::X86_64SysV,
    )
    .expect("inline optional element lists must lower through x86-64");
    assert!(artifact.assembly.contains("call ska_rt_alloc"));
    assert!(artifact
        .assembly
        .contains("call .Lska.class.main.Item.c0.init.i0"));
}

#[test]
fn nested_inline_array_element_lists_cross_the_complete_driver_pipeline() {
    let artifact = compile_source_to_assembly(
        "array-element-list.ska",
        concat!(
            "fn main() -> i64 {\n",
            "  var inner: i64[] = i64[]{1, 2};\n",
            "  var values: i64[][] = i64[][]{inner, i64[]{3}};\n",
            "  return values[1][0];\n",
            "}\n",
        ),
        Target::X86_64SysV,
    )
    .expect("nested inline-array element lists must lower through x86-64");
    assert!(artifact.assembly.contains("call .Lska_array_0_clone"));
    assert!(artifact.assembly.contains("call .Lska_array_0_release"));
}

#[test]
fn owner_element_list_families_cross_the_complete_driver_pipeline() {
    let artifact = compile_source_to_assembly(
        "array-element-list.ska",
        concat!(
            "class Item { value: i64; init(value: i64) { self.value = value; } }\n",
            "fn main() -> i64 {\n",
            "  var owner: shared Item = new Item(20);\n",
            "  var values: (shared Item)[] = (shared Item)[]{owner, new Item(22)};\n",
            "  var maybe: shared (shared? Item)[] = new (shared? Item)[]{none, owner};\n",
            "  var first: shared Item = values[0];\n",
            "  return first->value + (i64) maybe->len();\n",
            "}\n",
        ),
        Target::X86_64SysV,
    )
    .expect("shared-owner element lists must lower through x86-64");
    assert!(artifact.assembly.contains("call ska_rt_alloc"));
    assert!(artifact.assembly.contains("call ska_rt_free"));
}

#[test]
fn primitive_static_programs_cross_the_complete_driver_pipeline() {
    let artifact = compile_source_to_assembly(
        "static-field.ska",
        concat!(
            "class State { static count: i64; init() {} }\n",
            "fn main() -> i64 { State.count = 1; return State.count; }\n",
        ),
        Target::X86_64SysV,
    )
    .expect("primitive static fields must lower through x86-64");
    assert!(artifact
        .assembly
        .contains(".Lska.class.main.State.c0.static.s0"));
    assert!(artifact.assembly.contains("\n.bss\n"));
}

#[test]
fn synthesized_static_initializers_stop_before_unavailable_backend_startup() {
    let CompilationError::Diagnostics(report) = compile_source_to_assembly(
        "static-initializer.ska",
        concat!(
            "class State { static count: i64 = 42; init() {} }\n",
            "fn main() -> i64 { return 0; }\n",
        ),
        Target::X86_64SysV,
    )
    .unwrap_err() else {
        panic!("verified static initializer must stop at the backend startup boundary");
    };

    assert_eq!(report.diagnostics.len(), 1);
    let diagnostic = report.diagnostics.iter().next().unwrap();
    assert_eq!(
        diagnostic.code,
        STATIC_INITIALIZER_REQUIRES_LIFECYCLE_SYNTHESIS
    );
    assert!(diagnostic.labels.iter().any(|label| label
        .message
        .contains("final lifecycle coordinator MIR is verified")));
}

#[test]
fn static_lifetime_cycles_are_reported_as_source_diagnostics_before_synthesis() {
    let CompilationError::Diagnostics(report) = compile_source_to_assembly(
        "static-cycle.ska",
        concat!(
            "fn read_left() -> i64 { return State.left; }\n",
            "fn read_right() -> i64 { return State.right; }\n",
            "class State {\n",
            "  static left: i64 = read_right();\n",
            "  static right: i64 = read_left();\n",
            "  init() {}\n",
            "}\n",
            "fn main() -> i64 { return 0; }\n",
        ),
        Target::X86_64SysV,
    )
    .unwrap_err() else {
        panic!("static lifetime cycles must be ordinary source diagnostics");
    };

    assert_eq!(report.diagnostics.len(), 1);
    let diagnostic = report.diagnostics.iter().next().unwrap();
    assert_eq!(
        diagnostic.code,
        crate::passes::static_lifecycle::STATIC_LIFECYCLE_DEPENDENCY_CYCLE
    );
    assert!(diagnostic
        .labels
        .iter()
        .any(|label| label.message.contains("DirectCall")));
}
