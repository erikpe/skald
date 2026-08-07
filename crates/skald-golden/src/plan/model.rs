use crate::{ExitExpectation, MatchMode};
use std::{
    collections::BTreeMap,
    ffi::OsString,
    path::{Path, PathBuf},
};

/// One completely validated, immutable planning result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TestPlan {
    pub(super) golden_root: PathBuf,
    pub(super) artifact_root: PathBuf,
    pub(super) specs: Vec<PlannedSpec>,
    pub(super) tests: Vec<PlannedTest>,
    pub(super) builds: Vec<PlannedBuild>,
    pub(super) leaves: Vec<PlannedLeaf>,
}

impl TestPlan {
    pub fn golden_root(&self) -> &Path {
        &self.golden_root
    }

    pub fn artifact_root(&self) -> &Path {
        &self.artifact_root
    }

    pub fn specs(&self) -> &[PlannedSpec] {
        &self.specs
    }

    pub fn tests(&self) -> &[PlannedTest] {
        &self.tests
    }

    pub fn builds(&self) -> &[PlannedBuild] {
        &self.builds
    }

    pub fn leaves(&self) -> &[PlannedLeaf] {
        &self.leaves
    }

    pub fn leaf(&self, id: &str) -> Option<&PlannedLeaf> {
        self.leaves
            .binary_search_by_key(&id, |leaf| leaf.id.as_str())
            .ok()
            .map(|index| &self.leaves[index])
    }

    pub fn build(&self, id: &str) -> Option<&PlannedBuild> {
        self.builds
            .binary_search_by_key(&id, |build| build.id.as_str())
            .ok()
            .map(|index| &self.builds[index])
    }

    pub fn test(&self, id: &str) -> Option<&PlannedTest> {
        self.tests
            .binary_search_by_key(&id, |test| test.id.as_str())
            .ok()
            .map(|index| &self.tests[index])
    }
}

/// One discovered specification file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedSpec {
    pub(super) id: String,
    pub(super) path: PathBuf,
    pub(super) relative_path: String,
}

impl PlannedSpec {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn relative_path(&self) -> &str {
        &self.relative_path
    }
}

/// One source-backed test before compiler variants are expanded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedTest {
    pub(super) id: String,
    pub(super) spec_id: String,
    pub(super) name: String,
    pub(super) source: Option<PathBuf>,
    pub(super) source_relative: Option<String>,
    pub(super) build_ids: Vec<String>,
}

impl PlannedTest {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn spec_id(&self) -> &str {
        &self.spec_id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn source(&self) -> Option<&Path> {
        self.source.as_deref()
    }

    pub fn source_relative(&self) -> Option<&str> {
        self.source_relative.as_deref()
    }

    pub fn build_ids(&self) -> &[String] {
        &self.build_ids
    }
}

/// One independently compiled `(test, variant)` pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedBuild {
    pub(super) id: String,
    pub(super) test_id: String,
    pub(super) variant: String,
    pub(super) compiler_args: Vec<OsString>,
    pub(super) base_args: Vec<OsString>,
    pub(super) variant_args: Vec<OsString>,
    pub(super) command_line_args: Vec<OsString>,
    pub(super) artifact_directory: PathBuf,
    pub(super) timeout_seconds: Option<u64>,
    pub(super) serial: bool,
    pub(super) resources: Vec<String>,
    pub(super) leaf_ids: Vec<String>,
}

impl PlannedBuild {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn test_id(&self) -> &str {
        &self.test_id
    }

    pub fn variant(&self) -> &str {
        &self.variant
    }

    pub fn compiler_args(&self) -> &[OsString] {
        &self.compiler_args
    }

    pub fn base_args(&self) -> &[OsString] {
        &self.base_args
    }

    pub fn variant_args(&self) -> &[OsString] {
        &self.variant_args
    }

    pub fn command_line_args(&self) -> &[OsString] {
        &self.command_line_args
    }

    pub fn artifact_directory(&self) -> &Path {
        &self.artifact_directory
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

    pub fn leaf_ids(&self) -> &[String] {
        &self.leaf_ids
    }
}

/// One independently selectable run or compile-fail leaf.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedLeaf {
    pub(super) id: String,
    pub(super) spec_id: String,
    pub(super) spec_relative_path: String,
    pub(super) test_id: String,
    pub(super) build_id: String,
    pub(super) variant: String,
    pub(super) source_relative: Option<String>,
    pub(super) kind: PlannedLeafKind,
}

impl PlannedLeaf {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn spec_id(&self) -> &str {
        &self.spec_id
    }

    pub fn spec_relative_path(&self) -> &str {
        &self.spec_relative_path
    }

    pub fn test_id(&self) -> &str {
        &self.test_id
    }

    pub fn build_id(&self) -> &str {
        &self.build_id
    }

    pub fn variant(&self) -> &str {
        &self.variant
    }

    pub fn source_relative(&self) -> Option<&str> {
        self.source_relative.as_deref()
    }

    pub fn kind(&self) -> &PlannedLeafKind {
        &self.kind
    }
}

/// Mode-specific leaf data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlannedLeafKind {
    Run(Box<PlannedRun>),
    Compile(ResolvedCompileExpectation),
}

/// Fully resolved inputs and expectations for one native execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedRun {
    pub(super) name: String,
    pub(super) args: ResolvedArgs,
    pub(super) stdin: ResolvedByteSource,
    pub(super) input_files: Vec<ResolvedInputFile>,
    pub(super) cwd: ResolvedWorkingDirectory,
    pub(super) env: BTreeMap<String, String>,
    pub(super) timeout_seconds: Option<u64>,
    pub(super) serial: bool,
    pub(super) resources: Vec<String>,
    pub(super) expectation: ResolvedRunExpectation,
}

impl PlannedRun {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn args(&self) -> &ResolvedArgs {
        &self.args
    }

    pub fn stdin(&self) -> &ResolvedByteSource {
        &self.stdin
    }

    pub fn input_files(&self) -> &[ResolvedInputFile] {
        &self.input_files
    }

    pub fn cwd(&self) -> &ResolvedWorkingDirectory {
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

    pub fn expectation(&self) -> &ResolvedRunExpectation {
        &self.expectation
    }
}

/// UTF-8 arguments or a canonical exact-byte argument file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedArgs {
    Utf8(Vec<String>),
    File(PathBuf),
}

/// Inline UTF-8 data or a canonical exact-byte file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedByteSource {
    Inline(String),
    File(PathBuf),
}

/// One canonical temporary input-file source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedInputFile {
    pub(super) name: String,
    pub(super) contents: ResolvedByteSource,
}

impl ResolvedInputFile {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn contents(&self) -> &ResolvedByteSource {
        &self.contents
    }
}

/// The default private sandbox or a canonical read-only fixture directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedWorkingDirectory {
    Private,
    Fixture(PathBuf),
}

/// Fully resolved native observations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedRunExpectation {
    pub(super) exit: ExitExpectation,
    pub(super) stdout: ResolvedStreamExpectation,
    pub(super) stderr: ResolvedStreamExpectation,
    pub(super) output_files: Vec<ResolvedOutputFile>,
}

impl ResolvedRunExpectation {
    pub fn exit(&self) -> ExitExpectation {
        self.exit
    }

    pub fn stdout(&self) -> &ResolvedStreamExpectation {
        &self.stdout
    }

    pub fn stderr(&self) -> &ResolvedStreamExpectation {
        &self.stderr
    }

    pub fn output_files(&self) -> &[ResolvedOutputFile] {
        &self.output_files
    }
}

/// Fully resolved compiler stderr expectation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedCompileExpectation {
    pub(super) stderr: ResolvedStreamExpectation,
    pub(super) stderr_prefix_to_strip: Option<Vec<u8>>,
}

impl ResolvedCompileExpectation {
    pub fn stderr(&self) -> &ResolvedStreamExpectation {
        &self.stderr
    }

    /// An absolute fixture prefix removed before diagnostics are checked for
    /// determinism or compared with their expectation.
    pub fn stderr_prefix_to_strip(&self) -> Option<&[u8]> {
        self.stderr_prefix_to_strip.as_deref()
    }
}

/// Resolved stream policy and expected byte source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedStreamExpectation {
    Ignore,
    Match {
        mode: MatchMode,
        expected: ResolvedByteSource,
    },
}

impl ResolvedStreamExpectation {
    pub fn mode(&self) -> Option<MatchMode> {
        match self {
            Self::Ignore => None,
            Self::Match { mode, .. } => Some(*mode),
        }
    }

    pub fn expected(&self) -> Option<&ResolvedByteSource> {
        match self {
            Self::Ignore => None,
            Self::Match { expected, .. } => Some(expected),
        }
    }
}

/// One canonical temporary output-file expectation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedOutputFile {
    pub(super) name: String,
    pub(super) contents: ResolvedByteSource,
}

impl ResolvedOutputFile {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn contents(&self) -> &ResolvedByteSource {
        &self.contents
    }
}

pub(super) fn os_strings(values: &[String]) -> Vec<OsString> {
    values.iter().map(OsString::from).collect()
}
