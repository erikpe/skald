use crate::{
    PipeFailure, ProcessCommand, ProcessEnvironment, ProcessObservation, ProcessTermination,
    StreamMismatch,
};
use std::{path::PathBuf, time::Duration};

/// Cross-process reproducibility policy.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Determinism {
    #[default]
    Off,
    Compile,
    Full,
}

impl Determinism {
    pub(crate) fn compile_repetitions(self) -> usize {
        match self {
            Self::Off => 1,
            Self::Compile | Self::Full => 2,
        }
    }

    pub(crate) fn run_repetitions(self) -> usize {
        match self {
            Self::Off | Self::Compile => 1,
            Self::Full => 2,
        }
    }
}

impl std::str::FromStr for Determinism {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "off" => Ok(Self::Off),
            "compile" => Ok(Self::Compile),
            "full" => Ok(Self::Full),
            _ => Err(format!(
                "unknown determinism mode {value:?}; expected off, compile, or full"
            )),
        }
    }
}

/// Shared controls for real `skac` subprocesses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompilerConfig {
    executable: PathBuf,
    working_directory: PathBuf,
    environment: ProcessEnvironment,
    default_timeout: Duration,
}

impl CompilerConfig {
    pub fn new(executable: impl Into<PathBuf>, working_directory: impl Into<PathBuf>) -> Self {
        Self {
            executable: executable.into(),
            working_directory: working_directory.into(),
            environment: ProcessEnvironment::new(),
            default_timeout: Duration::from_secs(10),
        }
    }

    pub fn with_environment(mut self, environment: ProcessEnvironment) -> Self {
        self.environment = environment;
        self
    }

    pub fn with_default_timeout(mut self, timeout: Duration) -> Self {
        self.default_timeout = timeout;
        self
    }

    pub fn executable(&self) -> &std::path::Path {
        &self.executable
    }

    pub fn working_directory(&self) -> &std::path::Path {
        &self.working_directory
    }

    pub fn environment(&self) -> &ProcessEnvironment {
        &self.environment
    }

    pub fn default_timeout(&self) -> Duration {
        self.default_timeout
    }
}

/// The semantic compiler expectation for one expanded build.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompilationKind {
    Success,
    CompileFail,
}

/// One exact compiler subprocess and any emitted assembly bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompilerObservation {
    command: ProcessCommand,
    process: Option<ProcessObservation>,
    assembly_path: PathBuf,
    assembly: Option<Vec<u8>>,
}

impl CompilerObservation {
    pub(super) fn new(
        command: ProcessCommand,
        process: Option<ProcessObservation>,
        assembly_path: PathBuf,
        assembly: Option<Vec<u8>>,
    ) -> Self {
        Self {
            command,
            process,
            assembly_path,
            assembly,
        }
    }

    pub fn command(&self) -> &ProcessCommand {
        &self.command
    }

    pub fn process(&self) -> Option<&ProcessObservation> {
        self.process.as_ref()
    }

    pub fn assembly_path(&self) -> &std::path::Path {
        &self.assembly_path
    }

    pub fn assembly(&self) -> Option<&[u8]> {
        self.assembly.as_deref()
    }
}

/// One independently reportable compiler-stage defect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompilationIssue {
    Process(String),
    Termination {
        expected: i32,
        actual: ProcessTermination,
    },
    Pipe(PipeFailure),
    UnexpectedStdout(Vec<u8>),
    UnexpectedStderr(Vec<u8>),
    MissingAssembly(PathBuf),
    AssemblyRead {
        path: PathBuf,
        message: String,
    },
    NonUtf8Assembly(PathBuf),
    NondeterministicAssembly,
    NondeterministicDiagnostics,
    StderrExpectation(StreamMismatch),
    ExpectationLoad(String),
}

/// Complete observations and issues for one build's compilation stage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompilationExecution {
    build_id: String,
    kind: CompilationKind,
    observations: Vec<CompilerObservation>,
    issues: Vec<CompilationIssue>,
}

impl CompilationExecution {
    pub(super) fn new(
        build_id: String,
        kind: CompilationKind,
        observations: Vec<CompilerObservation>,
        issues: Vec<CompilationIssue>,
    ) -> Self {
        Self {
            build_id,
            kind,
            observations,
            issues,
        }
    }

    pub fn build_id(&self) -> &str {
        &self.build_id
    }

    pub fn kind(&self) -> CompilationKind {
        self.kind
    }

    pub fn observations(&self) -> &[CompilerObservation] {
        &self.observations
    }

    pub fn issues(&self) -> &[CompilationIssue] {
        &self.issues
    }

    pub fn passed(&self) -> bool {
        self.issues.is_empty()
    }

    pub fn first_assembly(&self) -> Option<&[u8]> {
        self.observations
            .first()
            .and_then(CompilerObservation::assembly)
    }
}
