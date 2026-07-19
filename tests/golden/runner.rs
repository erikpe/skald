//! Minimal native golden runner for the first vertical slice.

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{self, Command},
};

fn main() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let golden_root = repository.join("tests/golden/run");
    let build_root = repository.join("build/golden");
    let runtime_archive = repository.join("build/runtime/libskald_runtime.a");

    if let Err(message) = build_runtime(&repository) {
        eprintln!("golden: {message}");
        process::exit(1);
    }
    fs::create_dir_all(&build_root).expect("failed to create golden build directory");

    let mut sources = Vec::new();
    discover_sources(&golden_root, &mut sources).expect("failed to discover golden sources");
    sources.sort();
    if sources.is_empty() {
        eprintln!(
            "golden: no `.ska` cases found under {}",
            golden_root.display()
        );
        process::exit(1);
    }

    let mut failures = 0;
    for source in &sources {
        let relative = source.strip_prefix(&golden_root).unwrap();
        match run_case(source, relative, &build_root, &runtime_archive) {
            Ok(()) => println!("PASS tests/golden/run/{}", relative.display()),
            Err(message) => {
                failures += 1;
                eprintln!("FAIL tests/golden/run/{}\n{message}", relative.display());
            }
        }
    }

    println!(
        "golden: {}/{} native cases passed",
        sources.len() - failures,
        sources.len()
    );
    if failures != 0 {
        process::exit(1);
    }
}

fn build_runtime(repository: &Path) -> Result<(), String> {
    let result = Command::new("make")
        .arg("-C")
        .arg(repository.join("runtime"))
        .output()
        .map_err(|error| format!("could not start runtime build: {error}"))?;
    if result.status.success() {
        return Ok(());
    }
    Err(format!(
        "runtime build failed with {}: {}",
        display_status(result.status.code()),
        captured_error(&result.stderr, &result.stdout)
    ))
}

fn discover_sources(directory: &Path, sources: &mut Vec<PathBuf>) -> Result<(), std::io::Error> {
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

fn run_case(
    source: &Path,
    relative: &Path,
    build_root: &Path,
    runtime_archive: &Path,
) -> Result<(), String> {
    let expected_path = source.with_extension("exit");
    let expected_text = fs::read_to_string(&expected_path)
        .map_err(|error| format!("could not read {}: {error}", expected_path.display()))?;
    let expected: i32 = expected_text
        .trim()
        .parse()
        .map_err(|error| format!("invalid expected exit status: {error}"))?;
    if !(0..=255).contains(&expected) {
        return Err(format!(
            "expected exit status {expected} is outside 0..=255"
        ));
    }

    let flattened = relative
        .with_extension("")
        .to_string_lossy()
        .replace(['/', '\\'], "__");
    let executable = build_root.join(flattened);
    let compilation = Command::new(env!("CARGO_BIN_EXE_skac"))
        .arg(source)
        .arg("-o")
        .arg(&executable)
        .env("SKALD_RUNTIME_ARCHIVE", runtime_archive)
        .output()
        .map_err(|error| format!("could not start skac: {error}"))?;
    if !compilation.status.success() {
        return Err(format!(
            "compilation failed with {}: {}",
            display_status(compilation.status.code()),
            captured_error(&compilation.stderr, &compilation.stdout)
        ));
    }

    let execution = Command::new(&executable)
        .output()
        .map_err(|error| format!("could not run generated executable: {error}"))?;
    let actual = execution
        .status
        .code()
        .ok_or_else(|| "generated executable terminated by signal".to_owned())?;
    if actual != expected {
        return Err(format!(
            "exit status mismatch: expected {expected}, found {actual}\n{}",
            captured_error(&execution.stderr, &execution.stdout)
        ));
    }
    if !execution.stdout.is_empty() || !execution.stderr.is_empty() {
        return Err(format!(
            "first-slice executable produced unexpected output: {}",
            captured_error(&execution.stderr, &execution.stdout)
        ));
    }
    Ok(())
}

fn captured_error(stderr: &[u8], stdout: &[u8]) -> String {
    let stderr = String::from_utf8_lossy(stderr);
    let stdout = String::from_utf8_lossy(stdout);
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
