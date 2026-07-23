//! Deterministic compile-failure and native-execution golden runner.

mod native_expectations;

use std::{
    fs, io,
    path::{Path, PathBuf},
    process::{self, Command, Output},
};

use native_expectations::{load_native_expectations, verify_native_execution};

const COMPILE_FAILURE_EXIT_CODE: i32 = 1;

fn main() {
    if let Err(message) = run() {
        eprintln!("golden: {message}");
        process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let golden_root = repository.join("tests/golden");
    let run_root = golden_root.join("run");
    let compile_fail_root = golden_root.join("compile_fail");
    let build_root = repository.join("build/golden");
    let runtime_archive = repository.join("build/runtime/libskald_runtime.a");

    let run_sources = sorted_sources(&run_root)?;
    let compile_fail_sources = sorted_sources(&compile_fail_root)?;
    if run_sources.is_empty() && compile_fail_sources.is_empty() {
        return Err(format!(
            "no `.ska` cases found under {}",
            golden_root.display()
        ));
    }

    fs::create_dir_all(&build_root)
        .map_err(|error| format!("could not create golden build directory: {error}"))?;

    let mut failures = 0;
    for source in &run_sources {
        let relative = source
            .strip_prefix(&golden_root)
            .expect("discovered below root");
        match run_native_case(&repository, source, relative, &build_root, &runtime_archive) {
            Ok(()) => println!("PASS tests/golden/{}", relative.display()),
            Err(message) => {
                failures += 1;
                eprintln!("FAIL tests/golden/{}\n{message}", relative.display());
            }
        }
    }
    for source in &compile_fail_sources {
        let relative = source
            .strip_prefix(&golden_root)
            .expect("discovered below root");
        match run_compile_fail_case(&repository, source, relative, &build_root) {
            Ok(()) => println!("PASS tests/golden/{}", relative.display()),
            Err(message) => {
                failures += 1;
                eprintln!("FAIL tests/golden/{}\n{message}", relative.display());
            }
        }
    }

    let total = run_sources.len() + compile_fail_sources.len();
    println!(
        "golden: {}/{} cases passed ({} native, {} compile-fail)",
        total - failures,
        total,
        run_sources.len(),
        compile_fail_sources.len()
    );
    if failures == 0 {
        Ok(())
    } else {
        Err(format!("{failures} golden case(s) failed"))
    }
}

fn sorted_sources(directory: &Path) -> Result<Vec<PathBuf>, String> {
    let mut sources = Vec::new();
    discover_sources(directory, &mut sources)
        .map_err(|error| format!("could not discover {}: {error}", directory.display()))?;
    sources.sort();
    Ok(sources)
}

fn discover_sources(directory: &Path, sources: &mut Vec<PathBuf>) -> Result<(), io::Error> {
    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if path.is_dir() {
            discover_sources(&path, sources)?;
        } else if path.extension().is_some_and(|extension| extension == "ska") {
            sources.push(path);
        }
    }
    Ok(())
}

fn run_native_case(
    repository: &Path,
    source: &Path,
    relative: &Path,
    build_root: &Path,
    runtime_archive: &Path,
) -> Result<(), String> {
    let expected = load_native_expectations(source)?;

    assert_deterministic_assembly(repository, source, relative, build_root)?;

    let executable = build_root.join(flattened_stem(relative));
    let compilation = skac(repository, source)
        .args(["-o".as_ref(), executable.as_os_str()])
        .env("SKALD_RUNTIME_ARCHIVE", runtime_archive)
        .output()
        .map_err(|error| format!("could not start skac: {error}"))?;
    require_successful_compilation(&compilation)?;

    let execution = run_executable(&executable)?;
    let repeated = run_executable(&executable)?;
    if (
        execution.status.code(),
        &execution.stdout,
        &execution.stderr,
    ) != (repeated.status.code(), &repeated.stdout, &repeated.stderr)
    {
        return Err("native observation changed across two independent executions".to_owned());
    }
    verify_native_execution(
        &expected,
        execution.status.code(),
        &execution.stdout,
        &execution.stderr,
    )
}

fn run_executable(executable: &Path) -> Result<Output, String> {
    Command::new(executable)
        .output()
        .map_err(|error| format!("could not run generated executable: {error}"))
}

fn assert_deterministic_assembly(
    repository: &Path,
    source: &Path,
    relative: &Path,
    build_root: &Path,
) -> Result<(), String> {
    let stem = flattened_stem(relative);
    let first_path = build_root.join(format!("{stem}.first.s"));
    let second_path = build_root.join(format!("{stem}.second.s"));

    for output_path in [&first_path, &second_path] {
        let compilation = skac(repository, source)
            .args([
                "--emit".as_ref(),
                "asm".as_ref(),
                "-o".as_ref(),
                output_path.as_os_str(),
            ])
            .output()
            .map_err(|error| format!("could not start skac: {error}"))?;
        require_successful_compilation(&compilation)?;
    }

    let first = fs::read(&first_path)
        .map_err(|error| format!("could not read {}: {error}", first_path.display()))?;
    let second = fs::read(&second_path)
        .map_err(|error| format!("could not read {}: {error}", second_path.display()))?;
    if first != second {
        return Err("assembly changed across two independent compiler runs".to_owned());
    }
    Ok(())
}

fn run_compile_fail_case(
    repository: &Path,
    source: &Path,
    relative: &Path,
    build_root: &Path,
) -> Result<(), String> {
    let expected_path = source.with_extension("stderr");
    let expected = fs::read(&expected_path)
        .map_err(|error| format!("could not read {}: {error}", expected_path.display()))?;
    let output_path = build_root.join(format!("{}.unexpected.s", flattened_stem(relative)));

    let first = compile_failure(repository, source, &output_path)?;
    let second = compile_failure(repository, source, &output_path)?;
    if first.stderr != second.stderr {
        return Err("diagnostics changed across two independent compiler runs".to_owned());
    }
    if first.stderr != expected {
        return Err(format!(
            "diagnostic snapshot mismatch\nexpected:\n{}\nactual:\n{}",
            String::from_utf8_lossy(&expected),
            String::from_utf8_lossy(&first.stderr)
        ));
    }
    Ok(())
}

fn compile_failure(repository: &Path, source: &Path, output_path: &Path) -> Result<Output, String> {
    let result = skac(repository, source)
        .args([
            "--emit".as_ref(),
            "asm".as_ref(),
            "-o".as_ref(),
            output_path.as_os_str(),
        ])
        .output()
        .map_err(|error| format!("could not start skac: {error}"))?;
    if result.status.code() != Some(COMPILE_FAILURE_EXIT_CODE) {
        return Err(format!(
            "expected compiler exit status {COMPILE_FAILURE_EXIT_CODE}, found {}: {}",
            display_status(result.status.code()),
            captured_output(&result)
        ));
    }
    if !result.stdout.is_empty() {
        return Err(format!(
            "compile failure produced unexpected stdout: {}",
            String::from_utf8_lossy(&result.stdout)
        ));
    }
    Ok(result)
}

fn skac(repository: &Path, source: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_skac"));
    command.current_dir(repository).arg(
        source
            .strip_prefix(repository)
            .expect("golden source is inside the repository"),
    );
    command
}

fn require_successful_compilation(compilation: &Output) -> Result<(), String> {
    if !compilation.status.success() {
        return Err(format!(
            "compilation failed with {}: {}",
            display_status(compilation.status.code()),
            captured_output(compilation)
        ));
    }
    if !compilation.stdout.is_empty() || !compilation.stderr.is_empty() {
        return Err(format!(
            "successful compilation produced unexpected output: {}",
            captured_output(compilation)
        ));
    }
    Ok(())
}

fn flattened_stem(relative: &Path) -> String {
    relative
        .with_extension("")
        .to_string_lossy()
        .replace(['/', '\\'], "__")
}

fn captured_output(output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    match (stderr.trim(), stdout.trim()) {
        ("", "") => "<no output>".to_owned(),
        (stderr, "") => stderr.to_owned(),
        ("", stdout) => stdout.to_owned(),
        (stderr, stdout) => format!("stderr: {stderr}\nstdout: {stdout}"),
    }
}

fn display_status(code: Option<i32>) -> String {
    code.map_or_else(
        || "termination by signal".to_owned(),
        |code| format!("exit status {code}"),
    )
}
