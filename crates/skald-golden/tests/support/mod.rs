#![allow(dead_code)] // Each integration-test crate uses a different subset of shared fixtures.

use skald_compiler::driver::Toolchain;
use skald_golden::{
    build_plan, execute_sequential, select, CompilerConfig, Determinism, ExecutionOptions,
    ProcessCommand, ProcessEnvironment, RuntimePreparation, SelectionOptions, SequentialExecution,
    SequentialOptions,
};
use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);

pub struct Fixture {
    pub root: PathBuf,
    pub artifacts: PathBuf,
    pub temporary: PathBuf,
    pub runtime_archive: PathBuf,
    pub runtime_counter: PathBuf,
    pub link_counter: PathBuf,
    pub link_assembly: PathBuf,
}

impl Fixture {
    pub fn new() -> Self {
        let sequence = NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "skald-golden-fixture-{}-{sequence}",
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

    pub fn write(&self, relative: &str, contents: impl AsRef<[u8]>) {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, contents).unwrap();
    }

    pub fn plan(&self) -> skald_golden::TestPlan {
        build_plan(
            &self.root,
            &self.artifacts,
            &[OsString::from("--command-line"), OsString::from("last")],
        )
        .unwrap()
    }

    pub fn options(&self, determinism: Determinism, link_mode: &str) -> SequentialOptions {
        self.options_with_activity(determinism, link_mode, None)
    }

    pub fn execute(&self, determinism: Determinism) -> SequentialExecution {
        let plan = self.plan();
        let selected = select(&plan, &SelectionOptions::default()).unwrap();
        execute_sequential(&selected, &self.options(determinism, "success"))
    }

    pub fn options_with_activity(
        &self,
        determinism: Determinism,
        link_mode: &str,
        activity: Option<(&Path, &Path, u64)>,
    ) -> SequentialOptions {
        let mut environment = ProcessEnvironment::new();
        if let Some((active, peak, delay_ms)) = activity {
            environment.insert("SKALD_FAKE_ACTIVE", active.as_os_str());
            environment.insert("SKALD_FAKE_PEAK", peak.as_os_str());
            environment.insert("SKALD_FAKE_DELAY_MS", delay_ms.to_string());
        }
        let compiler = CompilerConfig::new(fake_compiler(), &self.root)
            .with_environment(environment.clone())
            .with_default_timeout(Duration::from_secs(5));
        let runtime = RuntimePreparation::new(
            ProcessCommand::new(fake_process(), &self.root)
                .with_arguments([
                    OsString::from("prepare-runtime"),
                    self.runtime_archive.as_os_str().to_owned(),
                    self.runtime_counter.as_os_str().to_owned(),
                ])
                .with_environment(environment.clone())
                .with_timeout(Duration::from_secs(5)),
            &self.runtime_archive,
        );
        let execution = ExecutionOptions::new(&self.temporary)
            .with_inherited_environment(environment.clone())
            .with_default_timeout(Duration::from_secs(5));
        environment.insert("SKALD_FAKE_LINK_MODE", link_mode);
        environment.insert("SKALD_FAKE_LINK_EXECUTABLE", fake_process().as_os_str());
        environment.insert("SKALD_FAKE_LINK_COUNT", self.link_counter.as_os_str());
        environment.insert(
            "SKALD_FAKE_LINK_ASSEMBLY_LOG",
            self.link_assembly.as_os_str(),
        );
        SequentialOptions::new(
            compiler,
            runtime,
            Toolchain::new(fake_linker(), &self.runtime_archive),
            execution,
        )
        .with_linker_environment(environment)
        .with_linker_timeout(Duration::from_secs(5))
        .with_determinism(determinism)
    }

    pub fn activity_paths(&self, name: &str) -> (PathBuf, PathBuf) {
        (
            self.root.join(format!(".{name}.active")),
            self.root.join(format!(".{name}.peak")),
        )
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

pub fn fake_compiler() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_skald-golden-fake-compiler"))
}

pub fn fake_linker() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_skald-golden-fake-linker"))
}

pub fn fake_process() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_skald-golden-fake-process"))
}

pub fn lines(path: &Path) -> usize {
    fs::read_to_string(path).unwrap().lines().count()
}

pub fn write_native_spec(fixture: &Fixture, mode: &str, run_body: &str) {
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

pub fn write_compile_fail_spec(fixture: &Fixture, mode: &str, expected: &str) {
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
