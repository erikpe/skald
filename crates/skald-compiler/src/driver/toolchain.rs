//! Host assembler/linker invocation for executable output.

use std::{
    env,
    ffi::{OsStr, OsString},
    fmt,
    io::{self, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use super::artifact::PendingArtifact;

pub const C_COMPILER_ENV: &str = "CC";
pub const RUNTIME_ARCHIVE_ENV: &str = "SKALD_RUNTIME_ARCHIVE";
pub const STANDARD_LIBRARY_ROOT_ENV: &str = "SKALD_STDLIB_ROOT";

const DEFAULT_RUNTIME_ARCHIVE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../build/runtime/libskald_runtime.a"
);
const DEFAULT_STANDARD_LIBRARY_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../std");

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Toolchain {
    c_compiler: OsString,
    runtime_archive: PathBuf,
    standard_library_root: PathBuf,
}

impl Toolchain {
    pub fn from_environment() -> Self {
        Self {
            c_compiler: env::var_os(C_COMPILER_ENV).unwrap_or_else(|| OsString::from("cc")),
            runtime_archive: env::var_os(RUNTIME_ARCHIVE_ENV)
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(DEFAULT_RUNTIME_ARCHIVE)),
            standard_library_root: env::var_os(STANDARD_LIBRARY_ROOT_ENV)
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(DEFAULT_STANDARD_LIBRARY_ROOT)),
        }
    }

    pub fn new(c_compiler: impl Into<OsString>, runtime_archive: impl Into<PathBuf>) -> Self {
        Self {
            c_compiler: c_compiler.into(),
            runtime_archive: runtime_archive.into(),
            standard_library_root: PathBuf::from(DEFAULT_STANDARD_LIBRARY_ROOT),
        }
    }

    pub fn with_standard_library_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.standard_library_root = root.into();
        self
    }

    pub fn standard_library_root(&self) -> &Path {
        &self.standard_library_root
    }

    pub fn link_assembly(&self, assembly: &str, output: &Path) -> Result<(), ToolchainError> {
        if !self.runtime_archive.is_file() {
            return Err(ToolchainError::RuntimeArchiveMissing);
        }

        let pending = PendingArtifact::new(output)
            .map_err(|source| ToolchainError::PrepareOutput { source })?;
        let mut child = Command::new(&self.c_compiler)
            .args([OsStr::new("-x"), OsStr::new("assembler"), OsStr::new("-")])
            .args([OsStr::new("-x"), OsStr::new("none")])
            .arg(&self.runtime_archive)
            .arg("-o")
            .arg(pending.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|source| ToolchainError::Start {
                tool: self.c_compiler.clone(),
                source,
            })?;

        let write_result = child
            .stdin
            .take()
            .expect("piped toolchain stdin must be available")
            .write_all(assembly.as_bytes());
        let result = child
            .wait_with_output()
            .map_err(|source| ToolchainError::Wait {
                tool: self.c_compiler.clone(),
                source,
            })?;
        if !result.status.success() {
            return Err(ToolchainError::Failed {
                tool: self.c_compiler.clone(),
                exit_code: result.status.code(),
                details: captured_output(&result.stderr, &result.stdout),
            });
        }
        if let Err(source) = write_result {
            return Err(ToolchainError::WriteAssembly {
                tool: self.c_compiler.clone(),
                source,
            });
        }

        pending
            .publish()
            .map_err(|source| ToolchainError::Publish { source })
    }
}

#[derive(Debug)]
pub enum ToolchainError {
    RuntimeArchiveMissing,
    PrepareOutput {
        source: io::Error,
    },
    Start {
        tool: OsString,
        source: io::Error,
    },
    WriteAssembly {
        tool: OsString,
        source: io::Error,
    },
    Wait {
        tool: OsString,
        source: io::Error,
    },
    Failed {
        tool: OsString,
        exit_code: Option<i32>,
        details: String,
    },
    Publish {
        source: io::Error,
    },
}

impl fmt::Display for ToolchainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RuntimeArchiveMissing => write!(
                formatter,
                "Skald runtime archive is unavailable; run `make runtime` or set {RUNTIME_ARCHIVE_ENV}"
            ),
            Self::PrepareOutput { source } => {
                write!(formatter, "could not prepare linked executable output: {source}")
            }
            Self::Start { tool, source } => write!(
                formatter,
                "could not start toolchain `{}`: {source}",
                tool.to_string_lossy()
            ),
            Self::WriteAssembly { tool, source } => write!(
                formatter,
                "could not send assembly to toolchain `{}`: {source}",
                tool.to_string_lossy()
            ),
            Self::Wait { tool, source } => write!(
                formatter,
                "could not wait for toolchain `{}`: {source}",
                tool.to_string_lossy()
            ),
            Self::Failed {
                tool,
                exit_code,
                details,
            } => {
                write!(
                    formatter,
                    "toolchain `{}` failed with {}",
                    tool.to_string_lossy(),
                    display_exit_status(*exit_code)
                )?;
                if !details.is_empty() {
                    write!(formatter, ": {details}")?;
                }
                Ok(())
            }
            Self::Publish { source } => {
                write!(formatter, "could not publish linked executable: {source}")
            }
        }
    }
}

impl std::error::Error for ToolchainError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::PrepareOutput { source }
            | Self::Start { source, .. }
            | Self::WriteAssembly { source, .. }
            | Self::Wait { source, .. }
            | Self::Publish { source } => Some(source),
            Self::RuntimeArchiveMissing | Self::Failed { .. } => None,
        }
    }
}

fn display_exit_status(exit_code: Option<i32>) -> String {
    exit_code.map_or_else(
        || "termination by signal".to_owned(),
        |code| format!("exit status {code}"),
    )
}

fn captured_output(stderr: &[u8], stdout: &[u8]) -> String {
    let stderr = String::from_utf8_lossy(stderr);
    let stdout = String::from_utf8_lossy(stdout);
    match (stderr.trim(), stdout.trim()) {
        ("", "") => String::new(),
        (stderr, "") => stderr.to_owned(),
        ("", stdout) => stdout.to_owned(),
        (stderr, stdout) => format!("stderr: {stderr}; stdout: {stdout}"),
    }
}
