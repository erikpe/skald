use skald_compiler::driver::Toolchain;
use skald_golden::{
    build_plan, execute_sequential, locate_compiler, run_process, select, CompilationIssue,
    CompilerConfig, Determinism, ExecutionOptions, ProcessCommand, ProcessEnvironment,
    ProcessTermination, RuntimePreparation, SelectionOptions, SequentialExecution,
    SequentialOptions, StageStatus,
};
use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);

struct Fixture {
    root: PathBuf,
    artifacts: PathBuf,
    temporary: PathBuf,
    runtime_archive: PathBuf,
    runtime_counter: PathBuf,
    link_counter: PathBuf,
    link_assembly: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "skald-golden-sequential-{}-{sequence}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("config.toml"), "schema=1\n").unwrap();
        Self {
            artifacts: root.with_extension("artifacts"),
            temporary: root.with_extension("temporary"),
            runtime_archive: root.with_extension("runtime.a"),
            runtime_counter: root.with_extension("runtime.count"),
            link_counter: root.with_extension("link.count"),
            link_assembly: root.with_extension("linked.s"),
            root,
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
        build_plan(
            &self.root,
            &self.artifacts,
            &[OsString::from("--command-line"), OsString::from("last")],
        )
        .unwrap()
    }

    fn execute(&self, determinism: Determinism) -> SequentialExecution {
        let plan = self.plan();
        let selected = select(&plan, &SelectionOptions::default()).unwrap();
        execute_sequential(&selected, &self.options(determinism, "success"))
    }

    fn options(&self, determinism: Determinism, link_mode: &str) -> SequentialOptions {
        let compiler = CompilerConfig::new(fake_compiler(), &self.root)
            .with_default_timeout(Duration::from_secs(5));
        let runtime = RuntimePreparation::new(
            ProcessCommand::new(fake_process(), &self.root)
                .with_arguments([
                    OsString::from("prepare-runtime"),
                    self.runtime_archive.as_os_str().to_owned(),
                    self.runtime_counter.as_os_str().to_owned(),
                ])
                .with_timeout(Duration::from_secs(5)),
            &self.runtime_archive,
        );
        let execution =
            ExecutionOptions::new(&self.temporary).with_default_timeout(Duration::from_secs(5));
        let mut linker_environment = ProcessEnvironment::new();
        linker_environment.insert("SKALD_FAKE_LINK_MODE", link_mode);
        linker_environment.insert("SKALD_FAKE_LINK_EXECUTABLE", fake_process().as_os_str());
        linker_environment.insert("SKALD_FAKE_LINK_COUNT", self.link_counter.as_os_str());
        linker_environment.insert(
            "SKALD_FAKE_LINK_ASSEMBLY_LOG",
            self.link_assembly.as_os_str(),
        );
        SequentialOptions::new(
            compiler,
            runtime,
            Toolchain::new(fake_linker(), &self.runtime_archive),
            execution,
        )
        .with_linker_environment(linker_environment)
        .with_linker_timeout(Duration::from_secs(5))
        .with_determinism(determinism)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        for path in [
            &self.root,
            &self.artifacts,
            &self.temporary,
            &self.runtime_archive,
            &self.runtime_counter,
            &self.link_counter,
            &self.link_assembly,
        ] {
            if path.is_dir() {
                fs::remove_dir_all(path).unwrap();
            } else if path.exists() {
                fs::remove_file(path).unwrap();
            }
        }
    }
}

fn fake_compiler() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_skald-golden-fake-compiler"))
}

fn fake_linker() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_skald-golden-fake-linker"))
}

fn fake_process() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_skald-golden-fake-process"))
}

fn lines(path: &Path) -> usize {
    fs::read_to_string(path).unwrap().lines().count()
}

fn write_native_spec(fixture: &Fixture, mode: &str, run_body: &str) {
    fixture.write("program.ska", "fn main() -> i64 { return 0; }\n");
    fixture.write(
        "native.golden.toml",
        format!(
            r#"schema=1
[[test]]
name="native"
mode="run"
source="program.ska"
compiler_args=["--fake-mode", "{mode}", "--base", "middle"]
[[test.run]]
name="run"
{run_body}
"#
        ),
    );
}

fn write_compile_fail_spec(fixture: &Fixture, mode: &str, expected: &str) {
    fixture.write("failure.ska", "fn main() -> i64 { return missing(); }\n");
    fixture.write(
        "failure.golden.toml",
        format!(
            r#"schema=1
[[test]]
name="failure"
mode="compile-fail"
source="failure.ska"
compiler_args=["--fake-mode", "{mode}"]
expect={{stderr={{match="contains", inline="{expected}"}}}}
"#
        ),
    );
}

#[test]
fn locates_explicit_compilers_and_rejects_missing_or_unusable_paths() {
    assert_eq!(
        locate_compiler(Some(fake_compiler())).unwrap(),
        fs::canonicalize(fake_compiler()).unwrap()
    );
    let fixture = Fixture::new();
    let missing = fixture.root.join("missing-skac");
    assert!(locate_compiler(Some(&missing))
        .unwrap_err()
        .to_string()
        .contains("could not resolve"));

    let unusable = fixture.root.join("not-executable");
    fs::write(&unusable, "not a compiler").unwrap();
    #[cfg(unix)]
    fs::set_permissions(&unusable, fs::Permissions::from_mode(0o600)).unwrap();
    assert!(locate_compiler(Some(&unusable))
        .unwrap_err()
        .to_string()
        .contains("not executable"));
}

#[test]
fn constructs_commands_in_source_base_variant_cli_and_runner_order() {
    let fixture = Fixture::new();
    fixture.write(
        "config.toml",
        "schema=1\n[variant.checked]\ncompiler_args=['--variant','after-base']\n",
    );
    fixture.write("program.ska", "fn main() -> i64 { return 0; }\n");
    fixture.write(
        "order.golden.toml",
        r#"schema=1
[[test]]
name="order"
mode="run"
source="program.ska"
variants=["checked"]
compiler_args=["--fake-mode","success","--base","middle"]
[[test.run]]
name="run"
args=["echo"]
"#,
    );
    let execution = fixture.execute(Determinism::Off);
    assert!(execution.passed());
    let arguments = execution.builds()[0].compilation().observations()[0]
        .command()
        .arguments();
    assert_eq!(
        arguments[0],
        fs::canonicalize(fixture.root.join("program.ska")).unwrap()
    );
    assert_eq!(
        &arguments[1..9],
        [
            "--fake-mode",
            "success",
            "--base",
            "middle",
            "--variant",
            "after-base",
            "--command-line",
            "last",
        ]
        .map(OsString::from)
    );
    assert_eq!(
        &arguments[9..12],
        ["--emit", "asm", "-o"].map(OsString::from)
    );
}

#[test]
fn determinism_modes_control_compiler_and_native_repetitions() {
    for (mode, compiler_count, run_count) in [
        (Determinism::Off, 1, 1),
        (Determinism::Compile, 2, 1),
        (Determinism::Full, 2, 2),
    ] {
        let fixture = Fixture::new();
        write_native_spec(
            &fixture,
            "success",
            "args=[\"echo\"]\nstdin={inline=\"hello\\n\"}\nexpect={stdout={inline=\"hello\\n\"},stderr={inline=\"hello\\n\"}}",
        );
        let execution = fixture.execute(mode);
        assert!(execution.passed(), "{mode:?}: {execution:#?}");
        assert_eq!(
            execution.builds()[0].compilation().observations().len(),
            compiler_count
        );
        assert_eq!(execution.leaves()[0].repetitions().len(), run_count);
        assert_eq!(fs::read_to_string(&fixture.runtime_counter).unwrap(), "1");
        assert_eq!(lines(&fixture.link_counter), 1);
        assert_eq!(
            fs::read(&fixture.link_assembly).unwrap(),
            execution.builds()[0]
                .compilation()
                .first_assembly()
                .unwrap()
        );
        assert!(execution.leaves()[0]
            .repetitions()
            .iter()
            .all(|run| !run.sandbox().exists()));
    }
}

#[test]
fn compile_fail_requires_status_stdout_stderr_and_deterministic_diagnostics() {
    let fixture = Fixture::new();
    write_compile_fail_spec(&fixture, "compile-fail", "error[FAKE001]");
    let execution = fixture.execute(Determinism::Compile);
    assert!(execution.passed(), "{execution:#?}");
    assert!(execution.runtime().is_none());
    assert_eq!(execution.builds()[0].compilation().observations().len(), 2);
    for observation in execution.builds()[0].compilation().observations() {
        let process = observation.process().unwrap();
        assert_eq!(process.termination(), ProcessTermination::Code(1));
        assert!(process.stdout().is_empty());
    }
    assert!(!fixture.runtime_counter.exists());
    assert!(!fixture.link_counter.exists());
}

#[test]
fn rejects_unexpected_compiler_output_status_and_missing_assembly() {
    for mode in ["unexpected-output", "status-two", "no-assembly"] {
        let fixture = Fixture::new();
        write_native_spec(&fixture, mode, "args=[\"echo\"]");
        let execution = fixture.execute(Determinism::Off);
        assert!(!execution.passed());
        let issues = execution.builds()[0].compilation().issues();
        match mode {
            "unexpected-output" => {
                assert!(issues
                    .iter()
                    .any(|issue| matches!(issue, CompilationIssue::UnexpectedStdout(_))));
                assert!(issues
                    .iter()
                    .any(|issue| matches!(issue, CompilationIssue::UnexpectedStderr(_))));
            }
            "status-two" => assert!(issues
                .iter()
                .any(|issue| matches!(issue, CompilationIssue::Termination { expected: 0, .. }))),
            "no-assembly" => assert!(issues
                .iter()
                .any(|issue| matches!(issue, CompilationIssue::MissingAssembly(_)))),
            _ => unreachable!(),
        }
        assert!(matches!(
            execution.leaves()[0].status(),
            StageStatus::Cancelled { .. }
        ));
    }
}

#[test]
fn reports_nondeterministic_assembly_diagnostics_and_native_output_files() {
    let fixture = Fixture::new();
    write_native_spec(&fixture, "nondeterministic-assembly", "args=[\"echo\"]");
    let execution = fixture.execute(Determinism::Compile);
    assert!(execution.builds()[0]
        .compilation()
        .issues()
        .contains(&CompilationIssue::NondeterministicAssembly));

    let fixture = Fixture::new();
    write_compile_fail_spec(&fixture, "nondeterministic-diagnostic", "error[FAKE001]");
    let execution = fixture.execute(Determinism::Compile);
    assert!(execution.builds()[0]
        .compilation()
        .issues()
        .contains(&CompilationIssue::NondeterministicDiagnostics));

    let fixture = Fixture::new();
    let counter = fixture.root.join("native.count");
    write_native_spec(
        &fixture,
        "success",
        &format!(
            "args=[\"write-vary-file\",\"{{tmp:output}}\",{:?}]\nexpect={{output_files=[{{name=\"output\",contents={{inline=\"1\"}}}}]}}",
            counter.display().to_string()
        ),
    );
    let execution = fixture.execute(Determinism::Full);
    assert!(matches!(
        execution.leaves()[0].status(),
        StageStatus::Failed(message) if message.contains("nondeterministic")
    ));
    assert!(execution.leaves()[0]
        .repetitions()
        .iter()
        .all(|run| run.retained() && run.sandbox().exists()));
}

#[test]
fn runtime_failure_cancels_only_native_links_and_runs() {
    let fixture = Fixture::new();
    write_native_spec(&fixture, "success", "args=[\"echo\"]");
    write_compile_fail_spec(&fixture, "compile-fail", "error[FAKE001]");
    let plan = fixture.plan();
    let selected = select(&plan, &SelectionOptions::default()).unwrap();
    let options = fixture.options(Determinism::Off, "success");
    let missing_runtime = RuntimePreparation::new(
        ProcessCommand::new(fake_process(), &fixture.root).with_arguments([OsString::from("echo")]),
        &fixture.runtime_archive,
    );
    let options = SequentialOptions::new(
        options.compiler().clone(),
        missing_runtime,
        Toolchain::new(fake_linker(), &fixture.runtime_archive),
        options.execution().clone(),
    );
    let execution = execute_sequential(&selected, &options);
    assert!(!execution.runtime().unwrap().status().passed());
    let native = execution
        .builds()
        .iter()
        .find(|build| build.build_id().contains("native"))
        .unwrap();
    assert!(native.compilation().passed());
    assert!(matches!(native.status(), StageStatus::Cancelled { .. }));
    let compile_fail = execution
        .builds()
        .iter()
        .find(|build| build.build_id().contains("failure"))
        .unwrap();
    assert!(compile_fail.status().passed());
}

#[test]
fn linker_failures_and_timeouts_cancel_runs_without_publishing_stale_executables() {
    for (mode, timeout) in [
        ("failure", Duration::from_secs(5)),
        ("sleep", Duration::from_millis(50)),
    ] {
        let fixture = Fixture::new();
        write_native_spec(&fixture, "success", "args=[\"echo\"]");
        let plan = fixture.plan();
        let selected = select(&plan, &SelectionOptions::default()).unwrap();
        let executable = plan.builds()[0].artifact_directory().join("program");
        fs::create_dir_all(executable.parent().unwrap()).unwrap();
        fs::write(&executable, "stale executable").unwrap();
        let options = fixture
            .options(Determinism::Off, mode)
            .with_linker_timeout(timeout);
        let execution = execute_sequential(&selected, &options);
        assert!(!execution.passed());
        assert!(!executable.exists());
        assert!(execution.builds()[0].link().unwrap().process().is_some());
        assert!(matches!(
            execution.leaves()[0].status(),
            StageStatus::Cancelled { .. }
        ));
        if mode == "sleep" {
            assert!(matches!(
                execution.builds()[0]
                    .link()
                    .unwrap()
                    .process()
                    .unwrap()
                    .termination(),
                ProcessTermination::TimedOut { .. }
            ));
        }
    }
}

#[test]
fn unrelated_builds_continue_after_a_compilation_failure() {
    let fixture = Fixture::new();
    write_native_spec(&fixture, "status-two", "args=[\"echo\"]");
    fixture.write("second.ska", "fn main() -> i64 { return 0; }\n");
    fixture.write(
        "second.golden.toml",
        r#"schema=1
[[test]]
name="second"
mode="run"
source="second.ska"
compiler_args=["--fake-mode","success"]
[[test.run]]
name="run"
args=["echo"]
"#,
    );
    let execution = fixture.execute(Determinism::Off);
    assert!(!execution.passed());
    assert!(execution
        .builds()
        .iter()
        .any(|build| build.build_id().contains("second") && build.status().passed()));
    assert!(execution
        .leaves()
        .iter()
        .any(|leaf| leaf.leaf_id().contains("second") && leaf.status().passed()));
}

#[test]
fn process_command_used_by_runtime_is_a_real_bounded_boundary() {
    let fixture = Fixture::new();
    let command = ProcessCommand::new(fake_process(), &fixture.root)
        .with_arguments([OsString::from("fail")])
        .with_timeout(Duration::from_secs(1));
    let observation = run_process(&command).unwrap();
    assert_eq!(observation.termination(), ProcessTermination::Code(17));
}
