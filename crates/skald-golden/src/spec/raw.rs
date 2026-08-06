use serde::Deserialize;
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawSpec {
    pub(super) schema: u64,
    #[serde(default)]
    pub(super) test: Vec<RawTest>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawConfig {
    pub(super) schema: u64,
    #[serde(default)]
    pub(super) variant: BTreeMap<String, RawVariant>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawVariant {
    #[serde(default)]
    pub(super) compiler_args: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum RawMode {
    Run,
    CompileFail,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawTest {
    pub(super) name: String,
    pub(super) mode: RawMode,
    pub(super) source: Option<PathBuf>,
    #[serde(default)]
    pub(super) compiler_args: Vec<String>,
    pub(super) variants: Option<Vec<String>>,
    pub(super) timeout: Option<u64>,
    #[serde(default)]
    pub(super) serial: bool,
    pub(super) resources: Option<Vec<String>>,
    #[serde(default)]
    pub(super) run: Vec<RawRun>,
    pub(super) expect: Option<RawCompileExpectation>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawRun {
    pub(super) name: String,
    pub(super) args: Option<Vec<String>>,
    pub(super) argv_file: Option<PathBuf>,
    pub(super) stdin: Option<RawByteSource>,
    pub(super) input_files: Option<Vec<RawInputFile>>,
    pub(super) cwd: Option<RawWorkingDirectory>,
    #[serde(default)]
    pub(super) env: BTreeMap<String, String>,
    pub(super) timeout: Option<u64>,
    #[serde(default)]
    pub(super) serial: bool,
    pub(super) resources: Option<Vec<String>>,
    pub(super) expect: Option<RawRunExpectation>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawByteSource {
    pub(super) inline: Option<String>,
    pub(super) file: Option<PathBuf>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawInputFile {
    pub(super) name: String,
    pub(super) contents: RawByteSource,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawWorkingDirectory {
    pub(super) fixture: PathBuf,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawRunExpectation {
    pub(super) exit: Option<RawExitExpectation>,
    pub(super) stdout: Option<RawStreamExpectation>,
    pub(super) stderr: Option<RawStreamExpectation>,
    pub(super) output_files: Option<Vec<RawOutputFileExpectation>>,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub(super) enum RawExitExpectation {
    Code(i32),
    Name(RawExitName),
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum RawExitName {
    Failure,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawCompileExpectation {
    pub(super) stderr: Option<RawStreamExpectation>,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum RawMatchMode {
    Exact,
    StartsWith,
    Contains,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawStreamExpectation {
    #[serde(rename = "match")]
    pub(super) mode: Option<RawMatchMode>,
    pub(super) inline: Option<String>,
    pub(super) file: Option<PathBuf>,
    pub(super) ignore: Option<bool>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawOutputFileExpectation {
    pub(super) name: String,
    pub(super) contents: RawByteSource,
}
