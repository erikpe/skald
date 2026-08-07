use skald_golden::{
    allowlisted_environment, build_plan, decode_arguments, execute_run, run_process,
    ExecutionOptions, PlannedLeafKind, PlannedRun, ProcessCommand, ProcessTermination, RunMismatch,
    SandboxRetention, StreamMatch,
};
use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
    thread,
    time::Duration,
};

#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;

static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);

struct Fixture {
    root: PathBuf,
    artifacts: PathBuf,
    temporary: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "skald-golden-process-{}-{sequence}",
            std::process::id()
        ));
        let artifacts = root.with_extension("artifacts");
        let temporary = root.with_extension("temporary");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("config.toml"), "schema = 1\n").unwrap();
        Self {
            root,
            artifacts,
            temporary,
        }
    }

    fn write(&self, relative: &str, contents: impl AsRef<[u8]>) {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, contents).unwrap();
    }

    fn plan(&self) -> skald_golden::TestPlan {
        build_plan(&self.root, &self.artifacts, &[]).unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        for path in [&self.root, &self.artifacts, &self.temporary] {
            if path.exists() {
                fs::remove_dir_all(path).unwrap();
            }
        }
    }
}

fn fake_process() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_skald-golden-fake-process"))
}

fn process(fake_arguments: &[OsString], stdin: Vec<u8>, cwd: &Path) -> ProcessCommand {
    ProcessCommand::new(fake_process(), cwd)
        .with_arguments(fake_arguments.iter().cloned())
        .with_stdin(stdin)
        .with_timeout(Duration::from_secs(5))
}

fn run<'a>(plan: &'a skald_golden::TestPlan, name: &str) -> &'a PlannedRun {
    plan.leaves()
        .iter()
        .find_map(|leaf| match leaf.kind() {
            PlannedLeafKind::Run(run) if run.name() == name => Some(run.as_ref()),
            PlannedLeafKind::Run(_) | PlannedLeafKind::Compile(_) => None,
        })
        .unwrap()
}

fn execution_options(fixture: &Fixture) -> ExecutionOptions {
    ExecutionOptions::new(&fixture.temporary).with_inherited_environment(allowlisted_environment())
}

#[test]
fn matches_inline_and_external_bytes_with_every_stream_policy() {
    let fixture = Fixture::new();
    fixture.write("program.ska", "fn main() -> i64 { return 0; }\n");
    fixture.write("expected.bin", b"external\0\xff\n\0");
    fixture.write("actual.txt", b"prefix middle suffix");
    fixture.write("prefix.txt", b"prefix");
    fixture.write("fragment.txt", b"middle");
    fixture.write(
        "streams.golden.toml",
        r#"schema = 1
[[test]]
name = "streams"
mode = "run"
source = "program.ska"

[[test.run]]
name = "exact-inline"
args = ["echo"]
stdin = { inline = "hello\u0000\u001b[31m\n" }
expect = { stdout = { match = "exact", inline = "hello\u0000\u001b[31m\n" }, stderr = { inline = "hello\u0000\u001b[31m\n" } }

[[test.run]]
name = "starts-inline"
args = ["echo"]
stdin = { inline = "prefix middle suffix" }
expect = { stdout = { match = "starts-with", inline = "prefix" }, stderr = { match = "starts-with", inline = "prefix" } }

[[test.run]]
name = "contains-inline"
args = ["echo"]
stdin = { inline = "prefix middle suffix" }
expect = { stdout = { match = "contains", inline = "middle" }, stderr = { match = "contains", inline = "middle" } }

[[test.run]]
name = "starts-external"
args = ["echo"]
stdin = { file = "actual.txt" }
expect = { stdout = { match = "starts-with", file = "prefix.txt" }, stderr = { match = "starts-with", file = "prefix.txt" } }

[[test.run]]
name = "contains-external"
args = ["echo"]
stdin = { file = "actual.txt" }
expect = { stdout = { match = "contains", file = "fragment.txt" }, stderr = { match = "contains", file = "fragment.txt" } }

[[test.run]]
name = "ignored"
args = ["fail"]
expect = { exit = "failure", stdout = { ignore = true }, stderr = { ignore = true } }

[[test.run]]
name = "exact-external"
args = ["echo"]
stdin = { file = "expected.bin" }
expect = { stdout = { file = "expected.bin" }, stderr = { file = "expected.bin" } }
"#,
    );
    let plan = fixture.plan();

    for name in [
        "exact-inline",
        "starts-inline",
        "contains-inline",
        "starts-external",
        "contains-external",
        "ignored",
        "exact-external",
    ] {
        let result = execute_run(
            fake_process(),
            run(&plan, name),
            &execution_options(&fixture),
        )
        .unwrap();
        assert!(result.passed(), "{name}: {:?}", result.mismatches());
        assert!(result.stdout_comparison().is_ok());
        assert!(result.stderr_comparison().is_ok());
        if name == "contains-inline" {
            assert!(matches!(
                result.stdout_comparison(),
                Ok(StreamMatch::Matched { offset: 7, .. })
            ));
        }
        assert!(!result.retained());
        assert!(!result.sandbox().exists());
    }
}

#[test]
fn preserves_exact_byte_arguments_including_empty_whitespace_and_non_utf8() {
    let fixture = Fixture::new();
    fixture.write("program.ska", "fn main() -> i64 { return 0; }\n");
    fixture.write(
        "arguments.argv",
        b"arguments\0\0space arg\0line\nfeed\0before\xffafter\0",
    );
    fixture.write(
        "expected.stdout",
        b"\0space arg\0line\nfeed\0before\xffafter\0",
    );
    fixture.write(
        "arguments.golden.toml",
        r#"schema=1
[[test]]
name="arguments"
mode="run"
source="program.ska"
[[test.run]]
name="binary"
argv_file="arguments.argv"
expect={stdout={file="expected.stdout"}}
"#,
    );
    let plan = fixture.plan();
    let result = execute_run(
        fake_process(),
        run(&plan, "binary"),
        &execution_options(&fixture),
    )
    .unwrap();
    assert!(result.passed(), "{:?}", result.mismatches());

    fixture.write("arguments.argv", b"unterminated");
    let error = decode_arguments(run(&plan, "binary").args()).unwrap_err();
    assert!(error.message().contains("must end with NUL"));
}

#[test]
fn prepares_temporary_files_and_substitutes_paths_in_arguments_and_stdin() {
    let fixture = Fixture::new();
    fixture.write("program.ska", "fn main() -> i64 { return 0; }\n");
    fixture.write("payload.bin", b"payload\0\xff\n");
    fixture.write(
        "temporary.golden.toml",
        r#"schema=1
[[test]]
name="temporary"
mode="run"
source="program.ska"

[[test.run]]
name="copy"
args=["copy-file", "{tmp:input}", "{tmp:output}"]
input_files=[{name="input", contents={file="payload.bin"}}]
expect={output_files=[{name="output", contents={file="payload.bin"}}]}

[[test.run]]
name="stdin-placeholder"
args=["echo"]
stdin={inline="{tmp:input}"}
input_files=[{name="input", contents={inline="unused"}}]
expect={stdout={ignore=true}, stderr={ignore=true}}

[[test.run]]
name="unknown-placeholder"
args=["echo", "{tmp:missing}"]
"#,
    );
    let plan = fixture.plan();
    let copy = execute_run(
        fake_process(),
        run(&plan, "copy"),
        &execution_options(&fixture),
    )
    .unwrap();
    assert!(copy.passed());
    assert_eq!(
        copy.output_files()[0].contents(),
        Some(b"payload\0\xff\n".as_slice())
    );

    let placeholder = execute_run(
        fake_process(),
        run(&plan, "stdin-placeholder"),
        &execution_options(&fixture),
    )
    .unwrap();
    assert!(placeholder.passed());
    assert_eq!(
        placeholder.observation().stdout(),
        path_bytes(&placeholder.sandbox().join("input"))
    );

    let error = execute_run(
        fake_process(),
        run(&plan, "unknown-placeholder"),
        &execution_options(&fixture),
    )
    .unwrap_err();
    assert!(error.message().contains("unknown temporary-path"));
    assert!(error.sandbox().unwrap().exists());
}

#[test]
fn temporary_placeholders_are_absolute_with_a_relative_temporary_root() {
    let fixture = Fixture::new();
    fixture.write("program.ska", "fn main() -> i64 { return 0; }\n");
    fixture.write(
        "temporary.golden.toml",
        r#"schema=1
[[test]]
name="temporary"
mode="run"
source="program.ska"

[[test.run]]
name="absolute"
args=["echo"]
stdin={inline="{tmp:input}"}
input_files=[{name="input", contents={inline="payload"}}]
expect={stdout={ignore=true}, stderr={ignore=true}}
"#,
    );
    let relative_root = PathBuf::from(format!(
        "target/skald-golden-relative-temporary-{}-{}",
        std::process::id(),
        NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
    ));
    assert!(!relative_root.is_absolute());

    let execution = execute_run(
        fake_process(),
        run(&fixture.plan(), "absolute"),
        &ExecutionOptions::new(&relative_root),
    )
    .unwrap();

    assert!(execution.passed());
    assert!(execution.sandbox().is_absolute());
    assert_eq!(
        execution.observation().stdout(),
        path_bytes(&execution.sandbox().join("input"))
    );
    fs::remove_dir_all(relative_root).unwrap();
}

#[test]
fn uses_fixture_working_directories_and_a_declared_environment() {
    let fixture = Fixture::new();
    fixture.write("program.ska", "fn main() -> i64 { return 0; }\n");
    fixture.write("fixture/.keep", "");
    fixture.write(
        "context.golden.toml",
        r#"schema=1
[[test]]
name="context"
mode="run"
source="program.ska"

[[test.run]]
name="cwd"
args=["cwd"]
cwd={fixture="fixture"}
expect={stdout={ignore=true}}

[[test.run]]
name="environment"
args=["env", "DECLARED_VALUE"]
env={DECLARED_VALUE="case-specific"}
expect={stdout={inline="case-specific"}}
"#,
    );
    let plan = fixture.plan();
    let cwd = execute_run(
        fake_process(),
        run(&plan, "cwd"),
        &execution_options(&fixture),
    )
    .unwrap();
    assert_eq!(
        cwd.observation().stdout(),
        path_bytes(&fs::canonicalize(fixture.root.join("fixture")).unwrap())
    );
    let environment = execute_run(
        fake_process(),
        run(&plan, "environment"),
        &execution_options(&fixture),
    )
    .unwrap();
    assert!(environment.passed());

    let empty_environment = process(
        &[OsString::from("env"), OsString::from("HOME")],
        Vec::new(),
        &fixture.root,
    );
    assert_eq!(
        run_process(&empty_environment).unwrap().stdout(),
        b"<unset>"
    );
    assert!(allowlisted_environment().get("HOME").is_none());
}

#[test]
fn concurrently_moves_data_larger_than_host_pipes() {
    let fixture = Fixture::new();
    let size = 2 * 1024 * 1024;
    let request = process(
        &[
            OsString::from("large-pipes"),
            OsString::from(size.to_string()),
        ],
        vec![b'i'; size],
        &fixture.root,
    );
    let observation = run_process(&request).unwrap();
    assert_eq!(observation.termination(), ProcessTermination::Code(0));
    assert_eq!(observation.stdout(), vec![b'o'; size]);
    assert_eq!(observation.stderr(), vec![b'e'; size]);
    assert!(observation.pipe_failures().is_empty());
}

#[test]
fn distinguishes_codes_signals_failures_and_timeouts() {
    let fixture = Fixture::new();
    let failure = run_process(&process(
        &[OsString::from("fail")],
        Vec::new(),
        &fixture.root,
    ))
    .unwrap();
    assert_eq!(failure.termination(), ProcessTermination::Code(17));
    assert_eq!(failure.stdout(), b"failure stdout\0\xff");
    assert_eq!(failure.stderr(), b"failure stderr\n");

    let signal = run_process(&process(
        &[OsString::from("signal")],
        Vec::new(),
        &fixture.root,
    ))
    .unwrap();
    assert_eq!(signal.termination(), ProcessTermination::Signal(15));

    let timeout = process(
        &[OsString::from("sleep"), OsString::from("60000")],
        Vec::new(),
        &fixture.root,
    )
    .with_timeout(Duration::from_millis(50));
    let timeout = run_process(&timeout).unwrap();
    assert_eq!(
        timeout.termination(),
        ProcessTermination::TimedOut {
            limit: Duration::from_millis(50)
        }
    );
    assert!(timeout.elapsed() < Duration::from_secs(5));
}

#[cfg(target_os = "linux")]
#[test]
fn timeout_terminates_descendants_in_the_child_process_group() {
    let fixture = Fixture::new();
    let request = process(&[OsString::from("descendant")], Vec::new(), &fixture.root)
        .with_timeout(Duration::from_millis(100));
    let observation = run_process(&request).unwrap();
    assert!(matches!(
        observation.termination(),
        ProcessTermination::TimedOut { .. }
    ));
    let pid = std::str::from_utf8(observation.stdout())
        .unwrap()
        .trim()
        .parse::<u32>()
        .unwrap();
    for _ in 0..100 {
        if !Path::new(&format!("/proc/{pid}")).exists() {
            return;
        }
        thread::sleep(Duration::from_millis(5));
    }
    panic!("timed-out descendant {pid} remained alive");
}

#[test]
fn deletes_passing_sandboxes_and_retains_failures_or_explicit_artifacts() {
    let fixture = Fixture::new();
    fixture.write("program.ska", "fn main() -> i64 { return 0; }\n");
    fixture.write(
        "retention.golden.toml",
        r#"schema=1
[[test]]
name="retention"
mode="run"
source="program.ska"

[[test.run]]
name="passing"
args=["echo"]

[[test.run]]
name="failing"
args=["fail"]

[[test.run]]
name="missing-output"
args=["echo"]
expect={output_files=[{name="missing", contents={inline="expected"}}]}
"#,
    );
    let plan = fixture.plan();
    let passing = execute_run(
        fake_process(),
        run(&plan, "passing"),
        &execution_options(&fixture),
    )
    .unwrap();
    assert!(passing.passed());
    assert!(!passing.retained());
    assert!(!passing.sandbox().exists());

    let failing = execute_run(
        fake_process(),
        run(&plan, "failing"),
        &execution_options(&fixture),
    )
    .unwrap();
    assert!(!failing.passed());
    assert!(failing.retained());
    assert!(failing.sandbox().exists());
    assert!(failing
        .mismatches()
        .iter()
        .any(|mismatch| matches!(mismatch, RunMismatch::Exit { .. })));

    let missing_output = execute_run(
        fake_process(),
        run(&plan, "missing-output"),
        &execution_options(&fixture),
    )
    .unwrap();
    assert!(missing_output.mismatches().iter().any(|mismatch| matches!(
        mismatch,
        RunMismatch::OutputFile(file) if file.actual().is_none()
    )));

    let keep_all = execution_options(&fixture).with_retention(SandboxRetention::All);
    let retained = execute_run(fake_process(), run(&plan, "passing"), &keep_all).unwrap();
    assert!(retained.passed());
    assert!(retained.retained());
    assert!(retained.sandbox().exists());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(retained.sandbox())
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
    }
}

#[cfg(unix)]
fn path_bytes(path: &Path) -> &[u8] {
    path.as_os_str().as_bytes()
}

#[cfg(not(unix))]
fn path_bytes(path: &Path) -> &[u8] {
    path.to_str().unwrap().as_bytes()
}
