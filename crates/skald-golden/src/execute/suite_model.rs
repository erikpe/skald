use crate::{
    CompilationExecution, CompilerConfig, Determinism, ExecutionOptions, ProcessCommand,
    ProcessEnvironment, ProcessObservation, RunExecution,
};
use skald_compiler::driver::Toolchain;
use std::{num::NonZeroUsize, path::PathBuf, time::Duration};

/// A stage's semantic result independent of final report formatting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StageStatus {
    Passed,
    Failed(String),
    Cancelled { dependency: String },
}

/// Controls bounded dependency-graph scheduling independently of stage policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SchedulerOptions {
    jobs: NonZeroUsize,
    fail_fast: bool,
}

impl SchedulerOptions {
    pub fn new(jobs: NonZeroUsize) -> Self {
        Self {
            jobs,
            fail_fast: false,
        }
    }

    pub fn with_fail_fast(mut self, fail_fast: bool) -> Self {
        self.fail_fast = fail_fast;
        self
    }

    pub fn jobs(self) -> NonZeroUsize {
        self.jobs
    }

    pub fn fail_fast(self) -> bool {
        self.fail_fast
    }
}

impl Default for SchedulerOptions {
    fn default() -> Self {
        Self::new(std::thread::available_parallelism().unwrap_or(NonZeroUsize::MIN))
    }
}

impl StageStatus {
    pub fn passed(&self) -> bool {
        matches!(self, Self::Passed)
    }
}

/// Runtime preparation command and expected archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimePreparation {
    command: ProcessCommand,
    archive: PathBuf,
}

impl RuntimePreparation {
    pub fn new(command: ProcessCommand, archive: impl Into<PathBuf>) -> Self {
        Self {
            command,
            archive: archive.into(),
        }
    }

    pub fn command(&self) -> &ProcessCommand {
        &self.command
    }

    pub fn archive(&self) -> &std::path::Path {
        &self.archive
    }
}

/// Shared compiler, runtime, linker, and native-run stage policy.
#[derive(Clone, Debug)]
pub struct StageOptions {
    compiler: CompilerConfig,
    runtime: RuntimePreparation,
    toolchain: Toolchain,
    execution: ExecutionOptions,
    linker_environment: ProcessEnvironment,
    linker_timeout: Duration,
    determinism: Determinism,
}

impl StageOptions {
    pub fn new(
        compiler: CompilerConfig,
        runtime: RuntimePreparation,
        toolchain: Toolchain,
        execution: ExecutionOptions,
    ) -> Self {
        let linker_environment = compiler.environment().clone();
        let linker_timeout = compiler.default_timeout();
        Self {
            compiler,
            runtime,
            toolchain,
            execution,
            linker_environment,
            linker_timeout,
            determinism: Determinism::Off,
        }
    }

    pub fn with_determinism(mut self, determinism: Determinism) -> Self {
        self.determinism = determinism;
        self
    }

    pub fn with_linker_environment(mut self, environment: ProcessEnvironment) -> Self {
        self.linker_environment = environment;
        self
    }

    pub fn with_linker_timeout(mut self, timeout: Duration) -> Self {
        self.linker_timeout = timeout;
        self
    }

    pub fn compiler(&self) -> &CompilerConfig {
        &self.compiler
    }

    pub fn runtime(&self) -> &RuntimePreparation {
        &self.runtime
    }

    pub fn toolchain(&self) -> &Toolchain {
        &self.toolchain
    }

    pub fn execution(&self) -> &ExecutionOptions {
        &self.execution
    }

    pub fn linker_environment(&self) -> &ProcessEnvironment {
        &self.linker_environment
    }

    pub fn linker_timeout(&self) -> Duration {
        self.linker_timeout
    }

    pub fn determinism(&self) -> Determinism {
        self.determinism
    }
}

/// Runtime Make observation, present only for native selections.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeExecution {
    command: ProcessCommand,
    archive: PathBuf,
    process: Option<ProcessObservation>,
    status: StageStatus,
}

impl RuntimeExecution {
    pub(super) fn new(
        command: ProcessCommand,
        archive: PathBuf,
        process: Option<ProcessObservation>,
        status: StageStatus,
    ) -> Self {
        Self {
            command,
            archive,
            process,
            status,
        }
    }

    pub fn command(&self) -> &ProcessCommand {
        &self.command
    }

    pub fn archive(&self) -> &std::path::Path {
        &self.archive
    }

    pub fn process(&self) -> Option<&ProcessObservation> {
        self.process.as_ref()
    }

    pub fn status(&self) -> &StageStatus {
        &self.status
    }
}

/// One Toolchain linkage attempt or dependency cancellation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkExecution {
    executable: PathBuf,
    command: Option<ProcessCommand>,
    process: Option<ProcessObservation>,
    status: StageStatus,
}

impl LinkExecution {
    pub(super) fn new(
        executable: PathBuf,
        command: Option<ProcessCommand>,
        process: Option<ProcessObservation>,
        status: StageStatus,
    ) -> Self {
        Self {
            executable,
            command,
            process,
            status,
        }
    }

    pub fn executable(&self) -> &std::path::Path {
        &self.executable
    }

    pub fn command(&self) -> Option<&ProcessCommand> {
        self.command.as_ref()
    }

    pub fn process(&self) -> Option<&ProcessObservation> {
        self.process.as_ref()
    }

    pub fn status(&self) -> &StageStatus {
        &self.status
    }
}

/// Compilation and optional linkage for one selected build variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildExecution {
    build_id: String,
    compilation: CompilationExecution,
    link: Option<LinkExecution>,
    status: StageStatus,
}

impl BuildExecution {
    pub(super) fn new(
        build_id: String,
        compilation: CompilationExecution,
        link: Option<LinkExecution>,
        status: StageStatus,
    ) -> Self {
        Self {
            build_id,
            compilation,
            link,
            status,
        }
    }

    pub fn build_id(&self) -> &str {
        &self.build_id
    }

    pub fn compilation(&self) -> &CompilationExecution {
        &self.compilation
    }

    pub fn link(&self) -> Option<&LinkExecution> {
        self.link.as_ref()
    }

    pub fn status(&self) -> &StageStatus {
        &self.status
    }
}

/// One selectable compile-fail or native-run leaf.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeafExecution {
    leaf_id: String,
    repetitions: Vec<RunExecution>,
    status: StageStatus,
}

impl LeafExecution {
    pub(super) fn new(
        leaf_id: String,
        repetitions: Vec<RunExecution>,
        status: StageStatus,
    ) -> Self {
        Self {
            leaf_id,
            repetitions,
            status,
        }
    }

    pub fn leaf_id(&self) -> &str {
        &self.leaf_id
    }

    pub fn repetitions(&self) -> &[RunExecution] {
        &self.repetitions
    }

    pub fn status(&self) -> &StageStatus {
        &self.status
    }
}

/// Backward-compatible name for the stage policy introduced with sequential execution.
pub type SequentialOptions = StageOptions;

/// Complete canonical-order result from the dependency plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanExecution {
    runtime: Option<RuntimeExecution>,
    builds: Vec<BuildExecution>,
    leaves: Vec<LeafExecution>,
    scheduler_failure: Option<SchedulerFailure>,
    elapsed: Duration,
}

impl PlanExecution {
    pub(super) fn new(
        runtime: Option<RuntimeExecution>,
        builds: Vec<BuildExecution>,
        leaves: Vec<LeafExecution>,
        scheduler_failure: Option<SchedulerFailure>,
        elapsed: Duration,
    ) -> Self {
        Self {
            runtime,
            builds,
            leaves,
            scheduler_failure,
            elapsed,
        }
    }

    pub fn runtime(&self) -> Option<&RuntimeExecution> {
        self.runtime.as_ref()
    }

    pub fn builds(&self) -> &[BuildExecution] {
        &self.builds
    }

    pub fn leaves(&self) -> &[LeafExecution] {
        &self.leaves
    }

    pub fn scheduler_failure(&self) -> Option<&SchedulerFailure> {
        self.scheduler_failure.as_ref()
    }

    pub fn elapsed(&self) -> Duration {
        self.elapsed
    }

    pub fn passed(&self) -> bool {
        self.scheduler_failure.is_none()
            && self
                .runtime
                .as_ref()
                .is_none_or(|runtime| runtime.status().passed())
            && self.builds.iter().all(|build| build.status().passed())
            && self.leaves.iter().all(|leaf| leaf.status().passed())
    }
}

/// Backward-compatible name for the result introduced with sequential execution.
pub type SequentialExecution = PlanExecution;

/// A scheduler infrastructure failure with a stable snapshot of unfinished work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchedulerFailure {
    message: String,
    active_nodes: Vec<String>,
    pending_nodes: Vec<String>,
}

impl SchedulerFailure {
    pub(super) fn new(
        message: impl Into<String>,
        active_nodes: Vec<String>,
        pending_nodes: Vec<String>,
    ) -> Self {
        Self {
            message: message.into(),
            active_nodes,
            pending_nodes,
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn active_nodes(&self) -> &[String] {
        &self.active_nodes
    }

    pub fn pending_nodes(&self) -> &[String] {
        &self.pending_nodes
    }
}
