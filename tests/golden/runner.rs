//! Deterministic compile-failure and native-execution golden runner.

mod native_expectations;

use std::{
    ffi::OsString,
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    process::{self, Command, Output, Stdio},
    thread,
};

use native_expectations::{load_native_expectations, verify_native_execution};

const COMPILE_FAILURE_EXIT_CODE: i32 = 1;
const CASE_ARGUMENTS_FILE: &str = "case.args";

#[derive(Debug)]
struct GoldenCase {
    expectation_stem: PathBuf,
    working_directory: PathBuf,
    arguments: Vec<OsString>,
    diagnostic_path_prefix: Option<Vec<u8>>,
}

impl GoldenCase {
    fn relative_to<'a>(&'a self, golden_root: &'a Path) -> &'a Path {
        self.expectation_stem
            .strip_prefix(golden_root)
            .expect("discovered below root")
    }

    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_skac"));
        command
            .current_dir(&self.working_directory)
            .args(&self.arguments);
        command
    }
}

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

    let run_cases = sorted_cases(&repository, &run_root)?;
    let compile_fail_cases = sorted_cases(&repository, &compile_fail_root)?;
    if run_cases.is_empty() && compile_fail_cases.is_empty() {
        return Err(format!(
            "no golden cases found under {}",
            golden_root.display()
        ));
    }

    fs::create_dir_all(&build_root)
        .map_err(|error| format!("could not create golden build directory: {error}"))?;

    let mut failures = 0;
    for case in &run_cases {
        let relative = case.relative_to(&golden_root);
        match run_native_case(case, relative, &build_root, &runtime_archive) {
            Ok(()) => println!("PASS tests/golden/{}", relative.display()),
            Err(message) => {
                failures += 1;
                eprintln!("FAIL tests/golden/{}\n{message}", relative.display());
            }
        }
    }
    for case in &compile_fail_cases {
        let relative = case.relative_to(&golden_root);
        match run_compile_fail_case(case, relative, &build_root) {
            Ok(()) => println!("PASS tests/golden/{}", relative.display()),
            Err(message) => {
                failures += 1;
                eprintln!("FAIL tests/golden/{}\n{message}", relative.display());
            }
        }
    }

    let total = run_cases.len() + compile_fail_cases.len();
    println!(
        "golden: {}/{} cases passed ({} native, {} compile-fail)",
        total - failures,
        total,
        run_cases.len(),
        compile_fail_cases.len()
    );
    if failures == 0 {
        Ok(())
    } else {
        Err(format!("{failures} golden case(s) failed"))
    }
}

fn sorted_cases(repository: &Path, directory: &Path) -> Result<Vec<GoldenCase>, String> {
    let mut cases = Vec::new();
    discover_cases(repository, directory, &mut cases)
        .map_err(|error| format!("could not discover {}: {error}", directory.display()))?;
    cases.sort_by(|left, right| left.expectation_stem.cmp(&right.expectation_stem));
    Ok(cases)
}

fn discover_cases(
    repository: &Path,
    directory: &Path,
    cases: &mut Vec<GoldenCase>,
) -> Result<(), io::Error> {
    let arguments_path = directory.join(CASE_ARGUMENTS_FILE);
    if arguments_path.is_file() {
        cases.push(load_multi_file_case(directory, &arguments_path)?);
        return Ok(());
    }

    for entry in fs::read_dir(directory)? {
        let path = entry?.path();
        if path.is_dir() {
            discover_cases(repository, &path, cases)?;
        } else if path.extension().is_some_and(|extension| extension == "ska") {
            cases.push(GoldenCase {
                expectation_stem: path.clone(),
                working_directory: repository.to_owned(),
                arguments: vec![path
                    .strip_prefix(repository)
                    .expect("golden source is inside the repository")
                    .as_os_str()
                    .to_owned()],
                diagnostic_path_prefix: None,
            });
        }
    }
    Ok(())
}

fn load_multi_file_case(directory: &Path, arguments_path: &Path) -> Result<GoldenCase, io::Error> {
    let text = fs::read_to_string(arguments_path)?;
    let arguments = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(OsString::from)
        .collect::<Vec<_>>();
    if arguments.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{} contains no arguments", arguments_path.display()),
        ));
    }

    let canonical_directory = fs::canonicalize(directory)?;
    Ok(GoldenCase {
        expectation_stem: arguments_path.to_owned(),
        working_directory: directory.to_owned(),
        arguments,
        diagnostic_path_prefix: Some(format!("{}/", canonical_directory.display()).into_bytes()),
    })
}

fn run_native_case(
    case: &GoldenCase,
    relative: &Path,
    build_root: &Path,
    runtime_archive: &Path,
) -> Result<(), String> {
    let expected = load_native_expectations(&case.expectation_stem)?;

    assert_deterministic_assembly(case, relative, build_root)?;

    let executable = build_root.join(flattened_stem(relative));
    let compilation = case
        .command()
        .args(["-o".as_ref(), executable.as_os_str()])
        .env("SKALD_RUNTIME_ARCHIVE", runtime_archive)
        .output()
        .map_err(|error| format!("could not start skac: {error}"))?;
    require_successful_compilation(&compilation)?;

    let execution = run_executable(&executable, &case.working_directory, expected.stdin())?;
    let repeated = run_executable(&executable, &case.working_directory, expected.stdin())?;
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

fn run_executable(
    executable: &Path,
    working_directory: &Path,
    input: &[u8],
) -> Result<Output, String> {
    let mut child = Command::new(executable)
        .current_dir(working_directory)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("could not start generated executable: {error}"))?;
    let mut stdin = child
        .stdin
        .take()
        .expect("piped child stdin is available exactly once");

    thread::scope(|scope| {
        let writer = scope.spawn(move || stdin.write_all(input));
        let output = child
            .wait_with_output()
            .map_err(|error| format!("could not wait for generated executable: {error}"));
        writer
            .join()
            .map_err(|_| "stdin writer thread panicked".to_owned())?
            .map_err(|error| format!("could not write generated executable stdin: {error}"))?;
        output
    })
}

fn assert_deterministic_assembly(
    case: &GoldenCase,
    relative: &Path,
    build_root: &Path,
) -> Result<(), String> {
    let stem = flattened_stem(relative);
    let first_path = build_root.join(format!("{stem}.first.s"));
    let second_path = build_root.join(format!("{stem}.second.s"));

    for output_path in [&first_path, &second_path] {
        let compilation = case
            .command()
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
    case: &GoldenCase,
    relative: &Path,
    build_root: &Path,
) -> Result<(), String> {
    let expected_path = case.expectation_stem.with_extension("stderr");
    let expected = fs::read(&expected_path)
        .map_err(|error| format!("could not read {}: {error}", expected_path.display()))?;
    let output_path = build_root.join(format!("{}.unexpected.s", flattened_stem(relative)));

    let first = compile_failure(case, &output_path)?;
    let second = compile_failure(case, &output_path)?;
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

fn compile_failure(case: &GoldenCase, output_path: &Path) -> Result<Output, String> {
    let mut result = case
        .command()
        .args([
            "--emit".as_ref(),
            "asm".as_ref(),
            "-o".as_ref(),
            output_path.as_os_str(),
        ])
        .output()
        .map_err(|error| format!("could not start skac: {error}"))?;
    if let Some(prefix) = &case.diagnostic_path_prefix {
        result.stderr = replace_bytes(&result.stderr, prefix, b"");
    }
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

fn replace_bytes(input: &[u8], needle: &[u8], replacement: &[u8]) -> Vec<u8> {
    if needle.is_empty() {
        return input.to_owned();
    }

    let mut output = Vec::with_capacity(input.len());
    let mut remaining = input;
    while let Some(index) = remaining
        .windows(needle.len())
        .position(|window| window == needle)
    {
        output.extend_from_slice(&remaining[..index]);
        output.extend_from_slice(replacement);
        remaining = &remaining[index + needle.len()..];
    }
    output.extend_from_slice(remaining);
    output
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
