mod support;

use std::{
    ffi::OsString,
    fs::{self, OpenOptions},
    os::unix::{ffi::OsStringExt, fs::PermissionsExt},
    path::Path,
    process::{Command, Output, Stdio},
};

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

    let help = Command::new(env!("CARGO_BIN_EXE_skac"))
        .arg("--help")
        .output()
        .unwrap();
    let help = String::from_utf8(help.stdout).unwrap();
    assert!(help.contains("-v, -q                  Increase or decrease operational report detail"));
    assert!(help.contains("--report-level <level>  Select off, phases, details, or trace reports"));
    assert!(help.contains("--diagnostic-level <l>  Render warning or error diagnostics"));
    assert!(help.contains("--mir-optimization <none|default>"));
    assert!(help.contains("--disable-mir-pass <name>"));
    assert!(help.contains("--list-mir-passes"));
}

#[test]
fn real_binary_lists_registered_mir_passes_without_an_input() {
    let output = Command::new(env!("CARGO_BIN_EXE_skac"))
        .arg("--list-mir-passes")
        .output()
        .unwrap();

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "Available final-MIR passes:\n  checked-integer-constant-folding [proof-rich]\n      Folds exact successful checked-integer constant protocols.\n  conservative-cfg-cleanup [proof-rich]\n      Folds ordinary branches and removes unprotected unreachable MIR blocks.\n  dead-pure-definition-elimination [proof-rich]\n      Removes unused non-failing scalar MIR definitions.\n  primitive-algebraic-simplification [proof-rich]\n      Simplifies exact primitive MIR algebraic identities.\n  primitive-constant-folding [proof-rich]\n      Folds exact block-local primitive MIR constants.\n  whole-world-reachability [final]\n      Removes unreachable executable MIR definitions.\n"
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn real_binary_honors_the_mir_optimization_selection_matrix() {
    let directory = TemporaryDirectory::new("mir-optimization-profiles").unwrap();
    let source = directory.join("main.ska");
    let default_assembly = directory.join("default.s");
    let none_assembly = directory.join("none.s");
    let all_disabled_assembly = directory.join("all-disabled.s");
    let constant_disabled_assembly = directory.join("constant-disabled.s");
    let duplicate_constant_disabled_assembly = directory.join("duplicate-constant-disabled.s");
    fs::write(&source, "fn main() -> i64 { return 6 * 7; }\n").unwrap();

    let default = Command::new(env!("CARGO_BIN_EXE_skac"))
        .arg(&source)
        .args([
            "--no-stdlib",
            "--emit",
            "asm",
            "--mir-optimization",
            "default",
            "-o",
        ])
        .arg(&default_assembly)
        .output()
        .unwrap();
    let none = Command::new(env!("CARGO_BIN_EXE_skac"))
        .arg(&source)
        .args([
            "--no-stdlib",
            "--emit",
            "asm",
            "--mir-optimization",
            "none",
            "-o",
        ])
        .arg(&none_assembly)
        .output()
        .unwrap();
    let all_disabled = Command::new(env!("CARGO_BIN_EXE_skac"))
        .arg(&source)
        .args([
            "--no-stdlib",
            "--emit",
            "asm",
            "--disable-mir-pass",
            "checked-integer-constant-folding",
            "--disable-mir-pass",
            "conservative-cfg-cleanup",
            "--disable-mir-pass",
            "dead-pure-definition-elimination",
            "--disable-mir-pass",
            "primitive-algebraic-simplification",
            "--disable-mir-pass",
            "primitive-constant-folding",
            "--disable-mir-pass",
            "whole-world-reachability",
            "-o",
        ])
        .arg(&all_disabled_assembly)
        .output()
        .unwrap();
    let constant_disabled = Command::new(env!("CARGO_BIN_EXE_skac"))
        .arg(&source)
        .args([
            "--no-stdlib",
            "--emit",
            "asm",
            "--disable-mir-pass",
            "primitive-constant-folding",
            "-o",
        ])
        .arg(&constant_disabled_assembly)
        .output()
        .unwrap();
    let duplicate_constant_disabled = Command::new(env!("CARGO_BIN_EXE_skac"))
        .arg(&source)
        .args([
            "--no-stdlib",
            "--emit",
            "asm",
            "--disable-mir-pass",
            "primitive-constant-folding",
            "--disable-mir-pass",
            "primitive-constant-folding",
            "-o",
        ])
        .arg(&duplicate_constant_disabled_assembly)
        .output()
        .unwrap();

    assert_same_process_output(&default, &none);
    assert_same_process_output(&default, &all_disabled);
    assert_same_process_output(&constant_disabled, &duplicate_constant_disabled);
    let default_assembly = fs::read(default_assembly).unwrap();
    let none_assembly = fs::read(none_assembly).unwrap();
    assert_ne!(default_assembly, none_assembly);
    assert_eq!(fs::read(all_disabled_assembly).unwrap(), none_assembly);
    assert_eq!(
        fs::read(duplicate_constant_disabled_assembly).unwrap(),
        fs::read(constant_disabled_assembly).unwrap()
    );
}

#[test]
fn real_binary_renders_the_detail_ladder_only_on_stderr() {
    let directory = TemporaryDirectory::new("report-levels").unwrap();
    let source = directory.join("main.ska");
    fs::write(&source, "fn main() -> i64 { return 42; }\n").unwrap();

    let cases = [
        ("phases", false, false),
        ("details", true, false),
        ("trace", true, true),
    ];
    for (level, has_details, has_trace) in cases {
        let assembly = directory.join(format!("{level}.s"));
        let output = Command::new(env!("CARGO_BIN_EXE_skac"))
            .arg(&source)
            .args([
                "--no-stdlib",
                "--emit",
                "asm",
                "--report-level",
                level,
                "-o",
            ])
            .arg(&assembly)
            .output()
            .expect("failed to execute skac");

        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(output.stdout.is_empty());
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains("skac: phase: module loading started\n"));
        assert!(stderr.contains("skac: artifact: assembly "));
        assert!(stderr.contains("skac: run: compilation completed"));
        assert!(stderr.contains("skac: run: driver completed"));
        assert_eq!(stderr.contains("skac: stats:"), has_details, "{level}");
        assert_eq!(stderr.contains(" completed in "), has_details, "{level}");
        assert_eq!(stderr.contains("skac: trace:"), has_trace, "{level}");
        assert!(assembly.is_file());
    }
}

#[test]
fn real_binary_default_output_matches_explicit_quiet_selection() {
    let directory = TemporaryDirectory::new("default-reporting").unwrap();
    let valid = directory.join("valid.ska");
    let invalid = directory.join("invalid.ska");
    fs::write(&valid, "fn main() -> i64 { return 42; }\n").unwrap();
    fs::write(&invalid, "fn main() -> i64 { return missing; }\n").unwrap();

    let default_assembly = directory.join("default.s");
    let explicit_assembly = directory.join("explicit.s");
    let default = Command::new(env!("CARGO_BIN_EXE_skac"))
        .arg(&valid)
        .args(["--no-stdlib", "--emit", "asm", "-o"])
        .arg(&default_assembly)
        .output()
        .unwrap();
    let explicit = Command::new(env!("CARGO_BIN_EXE_skac"))
        .arg(&valid)
        .args(["--no-stdlib", "--emit", "asm", "-o"])
        .arg(&explicit_assembly)
        .args(["--report-level", "off", "--diagnostic-level", "warning"])
        .output()
        .unwrap();
    assert_same_process_output(&default, &explicit);
    assert_eq!(
        fs::read(default_assembly).unwrap(),
        fs::read(explicit_assembly).unwrap()
    );

    let default = Command::new(env!("CARGO_BIN_EXE_skac"))
        .arg(&invalid)
        .args(["--no-stdlib", "--emit", "asm"])
        .output()
        .unwrap();
    let explicit = Command::new(env!("CARGO_BIN_EXE_skac"))
        .arg(&invalid)
        .args([
            "--no-stdlib",
            "--emit",
            "asm",
            "--report-level",
            "off",
            "--diagnostic-level",
            "warning",
        ])
        .output()
        .unwrap();
    assert_same_process_output(&default, &explicit);

    let missing_root = directory.join("missing-root");
    let default = Command::new(env!("CARGO_BIN_EXE_skac"))
        .args(["--entry", "app", "--module-root"])
        .arg(&missing_root)
        .args(["--no-stdlib", "--emit", "asm"])
        .output()
        .unwrap();
    let explicit = Command::new(env!("CARGO_BIN_EXE_skac"))
        .args(["--entry", "app", "--module-root"])
        .arg(&missing_root)
        .args([
            "--no-stdlib",
            "--emit",
            "asm",
            "--report-level",
            "off",
            "--diagnostic-level",
            "warning",
        ])
        .output()
        .unwrap();
    assert_same_process_output(&default, &explicit);

    let runtime = directory.join("runtime.a");
    fs::write(&runtime, "runtime").unwrap();
    let default_executable = directory.join("default-executable");
    let explicit_executable = directory.join("explicit-executable");
    let default = Command::new(env!("CARGO_BIN_EXE_skac"))
        .arg(&valid)
        .args(["--no-stdlib", "-o"])
        .arg(&default_executable)
        .env("CC", "false")
        .env("SKALD_RUNTIME_ARCHIVE", &runtime)
        .output()
        .unwrap();
    let explicit = Command::new(env!("CARGO_BIN_EXE_skac"))
        .arg(&valid)
        .args(["--no-stdlib", "-o"])
        .arg(&explicit_executable)
        .args(["--report-level", "off", "--diagnostic-level", "warning"])
        .env("CC", "false")
        .env("SKALD_RUNTIME_ARCHIVE", &runtime)
        .output()
        .unwrap();
    assert_same_process_output(&default, &explicit);

    for special in ["--help", "--version", "--list-mir-passes"] {
        let default = Command::new(env!("CARGO_BIN_EXE_skac"))
            .arg(special)
            .output()
            .unwrap();
        let explicit = Command::new(env!("CARGO_BIN_EXE_skac"))
            .args(["--report-level", "off", special])
            .output()
            .unwrap();
        assert_same_process_output(&default, &explicit);
    }
}

#[test]
fn real_binary_structurally_reports_terminal_failure_without_fixed_elapsed_time() {
    let directory = TemporaryDirectory::new("reported-failure").unwrap();
    let missing_root = directory.join("missing-root");

    let output = Command::new(env!("CARGO_BIN_EXE_skac"))
        .args(["--entry", "app", "--module-root"])
        .arg(&missing_root)
        .args(["--no-stdlib", "--emit", "asm", "--report-level", "details"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("skac: phase: provider normalization failed in "));
    assert!(stderr.contains("skac: run: compilation failed in "));
    assert!(stderr.contains("skac: run: driver failed in "));
    assert!(stderr.contains("skac: cannot normalize provider root"));
}

#[test]
fn real_binary_reports_executable_linking_publication_and_artifact() {
    let directory = TemporaryDirectory::new("executable-report").unwrap();
    let source = directory.join("main.ska");
    let executable = directory.join("main");
    let runtime = directory.join("runtime.a");
    let linker = directory.join("fake-linker.sh");
    fs::write(&source, "fn main() -> i64 { return 42; }\n").unwrap();
    fs::write(&runtime, "runtime").unwrap();
    fs::write(
        &linker,
        concat!(
            "#!/bin/sh\n",
            "output=\n",
            "while [ \"$#\" -gt 0 ]; do\n",
            "  if [ \"$1\" = \"-o\" ]; then output=$2; shift 2; else shift; fi\n",
            "done\n",
            "cat >/dev/null\n",
            "printf 'linked executable' >\"$output\"\n",
        ),
    )
    .unwrap();
    let mut permissions = fs::metadata(&linker).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&linker, permissions).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_skac"))
        .arg(&source)
        .args(["--no-stdlib", "-v", "-o"])
        .arg(&executable)
        .env("CC", &linker)
        .env("SKALD_RUNTIME_ARCHIVE", &runtime)
        .output()
        .expect("failed to execute skac");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("skac: phase: host linking completed\n"));
    assert!(stderr.contains("skac: phase: artifact publication completed\n"));
    assert!(stderr.contains(&format!(
        "skac: artifact: executable {}\n",
        executable.display()
    )));
    assert_eq!(fs::read_to_string(executable).unwrap(), "linked executable");
}

#[test]
fn report_writer_failure_maps_to_process_status_74_after_compilation() {
    let directory = TemporaryDirectory::new("report-writer-failure").unwrap();
    let source = directory.join("main.ska");
    let assembly = directory.join("main.s");
    fs::write(&source, "fn main() -> i64 { return 42; }\n").unwrap();
    let full = OpenOptions::new().write(true).open("/dev/full").unwrap();

    let status = Command::new(env!("CARGO_BIN_EXE_skac"))
        .arg(&source)
        .args(["--no-stdlib", "--emit", "asm", "-v", "-o"])
        .arg(&assembly)
        .stderr(Stdio::from(full))
        .status()
        .expect("failed to execute skac");

    assert_eq!(status.code(), Some(74));
    assert!(assembly.is_file());
}

#[test]
fn assembly_output_runs_the_real_pipeline_through_the_binary() {
    let directory = TemporaryDirectory::new("assembly").unwrap();
    let source = directory.join("answer.ska");
    let assembly = directory.join("answer.s");
    fs::write(&source, "fn main() -> i64 { return 6 * 7; }\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_skac"))
        .arg(&source)
        .args(["--no-stdlib", "--emit", "asm", "-o"])
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
    assert!(assembly_text.contains("mov rax, 42"));
    assert!(!assembly_text.contains("imul rax, rcx"));
    assert!(assembly_text.contains(".globl main"));
    assert!(assembly_text.contains("ska_rt_trace_top@tpoff"));
}

#[test]
fn omitted_runtime_trace_reaches_the_real_binary_pipeline() {
    let directory = TemporaryDirectory::new("assembly-omit-runtime-trace").unwrap();
    let source = directory.join("omitted_marker.ska");
    let assembly = directory.join("answer.s");
    fs::write(&source, "fn main() -> i64 { return 42; }\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_skac"))
        .arg(&source)
        .args(["--emit", "asm", "--omit-runtime-trace", "-o"])
        .arg(&assembly)
        .output()
        .expect("failed to execute skac");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let assembly = fs::read_to_string(&assembly).unwrap();
    assert!(!assembly.contains("ska_rt_trace_top"));
    assert!(!assembly.contains(".Lska.trace."));
    assert!(!assembly.contains("omitted_marker.ska"));
}

#[test]
fn missing_source_is_reported_as_a_compilation_error() {
    let output = Command::new(env!("CARGO_BIN_EXE_skac"))
        .arg("missing.ska")
        .output()
        .expect("failed to execute skac");

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8(output.stderr)
        .expect("skac stderr was not UTF-8")
        .contains("error[MOD001]: invalid entry"));
}

#[test]
fn logical_entry_compiles_reachable_modules_and_uses_its_leaf_output_default() {
    let directory = TemporaryDirectory::new("logical-entry").unwrap();
    write_module(
        directory.path(),
        "application modules/app/main.ska",
        "import lib::answer;\nfn main() -> i64 { return lib::answer::value(); }\n",
    );
    write_module(
        directory.path(),
        "dependency modules/lib/answer.ska",
        "public fn value() -> i64 { return 42; }\n",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_skac"))
        .current_dir(directory.path())
        .args([
            "--emit",
            "asm",
            "--module-root",
            "dependency modules",
            "--entry",
            "app::main",
            "--module-root",
            "application modules",
            "--no-stdlib",
        ])
        .output()
        .expect("failed to execute skac");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    let assembly = fs::read_to_string(directory.join("main.s")).unwrap();
    assert!(assembly.contains("call .Lska.fn.lib.answer.value.f1"));
    assert!(assembly.contains(".globl main"));
}

#[test]
fn positional_entry_and_module_root_paths_may_be_relative_and_contain_spaces() {
    let directory = TemporaryDirectory::new("positional-spaces").unwrap();
    write_module(
        directory.path(),
        "root with spaces/app/main.ska",
        "import lib::answer;\nfn main() -> i64 { return lib::answer::value(); }\n",
    );
    write_module(
        directory.path(),
        "root with spaces/lib/answer.ska",
        "public fn value() -> i64 { return 42; }\n",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_skac"))
        .current_dir(directory.path())
        .args([
            "--module-root",
            "root with spaces",
            "root with spaces/app/main.ska",
            "--no-stdlib",
            "--emit",
            "asm",
        ])
        .output()
        .expect("failed to execute skac");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(directory.join("root with spaces/app/main.s").is_file());
}

#[test]
fn installed_standard_library_root_is_injectable_through_the_real_binary() {
    let directory = TemporaryDirectory::new("installed-stdlib").unwrap();
    write_module(
        directory.path(),
        "modules/app/main.ska",
        "import std::answer;\nfn main() -> i64 { return std::answer::value(); }\n",
    );
    write_module(
        directory.path(),
        "injected sdk/std/answer.ska",
        "public fn value() -> i64 { return 42; }\n",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_skac"))
        .current_dir(directory.path())
        .env("SKALD_STDLIB_ROOT", "injected sdk")
        .args([
            "--entry",
            "app::main",
            "--module-root",
            "modules",
            "--emit",
            "asm",
        ])
        .output()
        .expect("failed to execute skac");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(fs::read_to_string(directory.join("main.s"))
        .unwrap()
        .contains("call .Lska.fn.std.answer.value.f1"));
}

#[test]
fn replacement_and_disabled_standard_library_options_control_lookup() {
    let directory = TemporaryDirectory::new("stdlib-options").unwrap();
    write_module(
        directory.path(),
        "modules/app/main.ska",
        "import std::answer;\nfn main() -> i64 { return std::answer::value(); }\n",
    );
    write_module(
        directory.path(),
        "replacement/std/answer.ska",
        "public fn value() -> i64 { return 42; }\n",
    );

    let replacement = Command::new(env!("CARGO_BIN_EXE_skac"))
        .current_dir(directory.path())
        .args([
            "--stdlib-root",
            "replacement",
            "--emit",
            "asm",
            "--entry",
            "app::main",
            "--module-root",
            "modules",
            "-o",
            "replacement.s",
        ])
        .output()
        .expect("failed to execute skac");
    assert!(
        replacement.status.success(),
        "{}",
        String::from_utf8_lossy(&replacement.stderr)
    );

    let disabled = Command::new(env!("CARGO_BIN_EXE_skac"))
        .current_dir(directory.path())
        .args([
            "--entry",
            "app::main",
            "--module-root",
            "modules",
            "--no-stdlib",
            "--emit",
            "asm",
        ])
        .output()
        .expect("failed to execute skac");
    assert_eq!(disabled.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&disabled.stderr)
        .contains("error[MOD003]: module `std::answer` was not found"));
}

#[test]
fn module_root_accepts_non_utf8_os_paths() {
    let directory = TemporaryDirectory::new("non-utf8-root").unwrap();
    let root_name = OsString::from_vec(b"modules-\xff".to_vec());
    let root = directory.join(&root_name);
    let output_name = OsString::from_vec(b"main-\xff.s".to_vec());
    fs::create_dir(&root).unwrap();
    write_module(&root, "app/main.ska", "fn main() -> i64 { return 42; }\n");

    let output = Command::new(env!("CARGO_BIN_EXE_skac"))
        .current_dir(directory.path())
        .arg("--module-root")
        .arg(root_name)
        .args([
            "--entry",
            "app::main",
            "--no-stdlib",
            "--emit",
            "asm",
            "-v",
            "-o",
        ])
        .arg(&output_name)
        .output()
        .expect("failed to execute skac");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(directory.join(output_name).is_file());
    assert!(String::from_utf8(output.stderr)
        .unwrap()
        .contains("skac: artifact: assembly main-�.s"));
}

fn write_module(base: &Path, relative: impl AsRef<Path>, source: &str) {
    let path = base.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, source).unwrap();
}

fn assert_same_process_output(default: &Output, explicit: &Output) {
    assert_eq!(default.status.code(), explicit.status.code());
    assert_eq!(default.stdout, explicit.stdout);
    assert_eq!(default.stderr, explicit.stderr);
}
