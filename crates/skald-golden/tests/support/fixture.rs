use super::{
    fake_tools::{fake_compiler, fake_linker, fake_process},
    temporary::TemporaryWorkspace,
};
use skald_compiler::driver::Toolchain;
use skald_golden::{
    build_plan, execute_sequential, select, CompilerConfig, Determinism, ExecutionOptions,
    ProcessCommand, ProcessEnvironment, RuntimePreparation, SelectionOptions, SequentialExecution,
    SequentialOptions,
};
use std::{ffi::OsString, fs, path::PathBuf, time::Duration};

const ASSOCIATED_PATHS: &[&str] = &[
    "artifacts",
    "temporary",
    "runtime.a",
    "runtime.count",
    "link.count",
    "linked.s",
];

pub(crate) struct Fixture {
    workspace: TemporaryWorkspace,
    pub(crate) root: PathBuf,
    pub(crate) artifacts: PathBuf,
    pub(crate) temporary: PathBuf,
    pub(crate) runtime_archive: PathBuf,
    pub(crate) runtime_counter: PathBuf,
    pub(crate) link_counter: PathBuf,
    pub(crate) link_assembly: PathBuf,
}

impl Fixture {
    pub(crate) fn new() -> Self {
        let workspace = TemporaryWorkspace::new("fixture", ASSOCIATED_PATHS);
        workspace.write("config.toml", "schema=1\n");
        Self {
            root: workspace.root().to_owned(),
            artifacts: workspace.associated_path("artifacts"),
            temporary: workspace.associated_path("temporary"),
            runtime_archive: workspace.associated_path("runtime.a"),
            runtime_counter: workspace.associated_path("runtime.count"),
            link_counter: workspace.associated_path("link.count"),
            link_assembly: workspace.associated_path("linked.s"),
            workspace,
        }
    }

    pub(crate) fn write(&self, relative: &str, contents: impl AsRef<[u8]>) {
        self.workspace.write(relative, contents);
    }

    pub(crate) fn plan(&self) -> skald_golden::TestPlan {
        build_plan(
            &self.root,
            &self.artifacts,
            &[OsString::from("--command-line"), OsString::from("last")],
        )
        .unwrap()
    }

    pub(crate) fn options(&self, determinism: Determinism, link_mode: &str) -> SequentialOptions {
        self.options_with_activity(determinism, link_mode, None)
    }

    #[allow(dead_code)] // Some integration roots inspect planning or options directly.
    pub(crate) fn execute(&self, determinism: Determinism) -> SequentialExecution {
        let plan = self.plan();
        let selected = select(&plan, &SelectionOptions::default()).unwrap();
        execute_sequential(&selected, &self.options(determinism, "success"))
    }

    pub(crate) fn options_with_activity(
        &self,
        determinism: Determinism,
        link_mode: &str,
        activity: Option<(&std::path::Path, &std::path::Path, u64)>,
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

    #[allow(dead_code)] // Only scheduler tests measure concurrent activity.
    pub(crate) fn activity_paths(&self, name: &str) -> (PathBuf, PathBuf) {
        (
            self.root.join(format!(".{name}.active")),
            self.root.join(format!(".{name}.peak")),
        )
    }
}

#[allow(dead_code)] // Only sequential orchestration checks counter cardinality.
pub(crate) fn lines(path: &std::path::Path) -> usize {
    fs::read_to_string(path).unwrap().lines().count()
}
