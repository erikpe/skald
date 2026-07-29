mod support;

use std::{ffi::OsString, fs, os::unix::ffi::OsStringExt, path::Path, process::Command};

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
    fs::create_dir(&root).unwrap();
    write_module(&root, "app/main.ska", "fn main() -> i64 { return 42; }\n");

    let output = Command::new(env!("CARGO_BIN_EXE_skac"))
        .current_dir(directory.path())
        .arg("--module-root")
        .arg(root_name)
        .args(["--entry", "app::main", "--no-stdlib", "--emit", "asm"])
        .output()
        .expect("failed to execute skac");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(directory.join("main.s").is_file());
}

fn write_module(base: &Path, relative: impl AsRef<Path>, source: &str) {
    let path = base.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, source).unwrap();
}
