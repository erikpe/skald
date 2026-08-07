use crate::{StreamMatcher, StreamMatcherSet};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

/// The validated schema version understood by this runner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SchemaVersion {
    /// The initial frozen golden-test schema.
    V1,
    /// Independent matcher collections for every declared process stream.
    V2,
}

/// A validated golden specification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spec {
    pub(super) schema: SchemaVersion,
    pub(super) tests: Vec<Test>,
}

impl Spec {
    pub fn schema(&self) -> SchemaVersion {
        self.schema
    }

    pub fn tests(&self) -> &[Test] {
        &self.tests
    }
}

/// Repository-wide named compiler variants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryConfig {
    pub(super) schema: SchemaVersion,
    pub(super) variants: BTreeMap<String, Variant>,
}

impl RepositoryConfig {
    pub fn schema(&self) -> SchemaVersion {
        self.schema
    }

    /// Returns all variants, including the implicit empty `default` variant.
    pub fn variants(&self) -> &BTreeMap<String, Variant> {
        &self.variants
    }
}

/// Compiler arguments contributed by one repository variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variant {
    pub(super) compiler_args: Vec<String>,
}

impl Variant {
    pub fn compiler_args(&self) -> &[String] {
        &self.compiler_args
    }
}

/// One source-backed run or compile-fail test.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Test {
    pub(super) name: String,
    pub(super) source: Option<PathBuf>,
    pub(super) compiler_args: Vec<String>,
    pub(super) variants: Vec<String>,
    pub(super) timeout_seconds: Option<u64>,
    pub(super) serial: bool,
    pub(super) resources: Vec<String>,
    pub(super) kind: TestKind,
}

impl Test {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn source(&self) -> Option<&Path> {
        self.source.as_deref()
    }

    pub fn compiler_args(&self) -> &[String] {
        &self.compiler_args
    }

    pub fn variants(&self) -> &[String] {
        &self.variants
    }

    pub fn timeout_seconds(&self) -> Option<u64> {
        self.timeout_seconds
    }

    pub fn serial(&self) -> bool {
        self.serial
    }

    pub fn resources(&self) -> &[String] {
        &self.resources
    }

    pub fn kind(&self) -> &TestKind {
        &self.kind
    }
}

/// Mode-specific validated test data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestKind {
    Run(RunTest),
    CompileFail(CompileFailTest),
}

/// Named executions that share one successful build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunTest {
    pub(super) runs: Vec<Run>,
}

impl RunTest {
    pub fn runs(&self) -> &[Run] {
        &self.runs
    }
}

/// Expectations for one rejected compilation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileFailTest {
    pub(super) expectation: CompileExpectation,
}

impl CompileFailTest {
    pub fn expectation(&self) -> &CompileExpectation {
        &self.expectation
    }
}

/// One named execution of a successfully built program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Run {
    pub(super) name: String,
    pub(super) args: ArgSource,
    pub(super) stdin: ByteSource,
    pub(super) input_files: Vec<InputFile>,
    pub(super) cwd: WorkingDirectory,
    pub(super) env: BTreeMap<String, String>,
    pub(super) timeout_seconds: Option<u64>,
    pub(super) serial: bool,
    pub(super) resources: Vec<String>,
    pub(super) expectation: RunExpectation,
}

impl Run {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn args(&self) -> &ArgSource {
        &self.args
    }

    pub fn stdin(&self) -> &ByteSource {
        &self.stdin
    }

    pub fn input_files(&self) -> &[InputFile] {
        &self.input_files
    }

    pub fn cwd(&self) -> &WorkingDirectory {
        &self.cwd
    }

    pub fn env(&self) -> &BTreeMap<String, String> {
        &self.env
    }

    pub fn timeout_seconds(&self) -> Option<u64> {
        self.timeout_seconds
    }

    pub fn serial(&self) -> bool {
        self.serial
    }

    pub fn resources(&self) -> &[String] {
        &self.resources
    }

    pub fn expectation(&self) -> &RunExpectation {
        &self.expectation
    }
}

/// UTF-8 arguments or an exact-byte argument file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArgSource {
    Utf8(Vec<String>),
    File(PathBuf),
}

/// Inline UTF-8 data or an external exact-byte file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ByteSource {
    Inline(String),
    File(PathBuf),
}

/// A temporary input file written before a native run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputFile {
    pub(super) name: String,
    pub(super) contents: ByteSource,
}

impl InputFile {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn contents(&self) -> &ByteSource {
        &self.contents
    }
}

/// The working directory selected for a native run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkingDirectory {
    Private,
    Fixture(PathBuf),
}

/// Expected observations for one native run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunExpectation {
    pub(super) exit: ExitExpectation,
    pub(super) stdout: StreamExpectation,
    pub(super) stderr: StreamExpectation,
    pub(super) output_files: Vec<OutputFileExpectation>,
}

impl RunExpectation {
    pub fn exit(&self) -> ExitExpectation {
        self.exit
    }

    pub fn stdout(&self) -> &StreamExpectation {
        &self.stdout
    }

    pub fn stderr(&self) -> &StreamExpectation {
        &self.stderr
    }

    pub fn output_files(&self) -> &[OutputFileExpectation] {
        &self.output_files
    }
}

/// The exact code or general failure expected from a native run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitExpectation {
    Code(i32),
    Failure,
}

/// Expected compiler streams for a compile-fail test.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompileExpectation {
    pub(super) stdout: StreamExpectation,
    pub(super) stderr: StreamExpectation,
}

impl CompileExpectation {
    pub fn stdout(&self) -> &StreamExpectation {
        &self.stdout
    }

    pub fn stderr(&self) -> &StreamExpectation {
        &self.stderr
    }
}

/// A whole-stream ignore policy or nonempty independent matcher collection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamExpectation {
    Ignore,
    Match(StreamMatcherSet<ByteSource>),
}

impl StreamExpectation {
    pub fn exact_empty() -> Self {
        Self::Match(StreamMatcherSet::one(StreamMatcher::new(
            MatchMode::Exact,
            ByteSource::Inline(String::new()),
        )))
    }

    pub fn matchers(&self) -> Option<&[StreamMatcher<ByteSource>]> {
        match self {
            Self::Ignore => None,
            Self::Match(matchers) => Some(matchers.matchers()),
        }
    }
}

/// Literal byte matching policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchMode {
    Exact,
    StartsWith,
    Contains,
}

/// One temporary output file compared after a native run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputFileExpectation {
    pub(super) name: String,
    pub(super) contents: ByteSource,
}

impl OutputFileExpectation {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn contents(&self) -> &ByteSource {
        &self.contents
    }
}
