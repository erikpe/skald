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
        self.link_assembly_with(assembly, output, execute_link)
    }

    /// Links assembly through a caller-provided process executor.
    ///
    /// The toolchain retains command construction, runtime validation, pending
    /// artifact ownership, failure interpretation, and atomic publication.
    /// Repository tooling may inject a bounded executor without duplicating
    /// those policies.
    pub fn link_assembly_with(
        &self,
        assembly: &str,
        output: &Path,
        execute: impl FnOnce(&LinkInvocation) -> Result<LinkObservation, ToolchainError>,
    ) -> Result<(), ToolchainError> {
        if !self.runtime_archive.is_file() {
            return Err(ToolchainError::RuntimeArchiveMissing);
        }

        let pending = PendingArtifact::new(output)
            .map_err(|source| ToolchainError::PrepareOutput { source })?;
        let invocation = LinkInvocation {
            program: self.c_compiler.clone(),
            arguments: vec![
                OsString::from("-x"),
                OsString::from("assembler"),
                OsString::from("-"),
                OsString::from("-x"),
                OsString::from("none"),
                self.runtime_archive.as_os_str().to_owned(),
                OsString::from("-o"),
                pending.path().as_os_str().to_owned(),
            ],
            stdin: assembly.as_bytes().to_vec(),
        };
        let result = execute(&invocation)?;
        if result.exit_code != Some(0) {
            return Err(ToolchainError::Failed {
                tool: self.c_compiler.clone(),
                exit_code: result.exit_code,
                details: captured_output(&result.stderr, &result.stdout),
            });
        }

        pending
            .publish()
            .map_err(|source| ToolchainError::Publish { source })
    }
}

/// One fully constructed host-linker invocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinkInvocation {
    program: OsString,
    arguments: Vec<OsString>,
    stdin: Vec<u8>,
}

impl LinkInvocation {
    pub fn program(&self) -> &OsStr {
        &self.program
    }

    pub fn arguments(&self) -> &[OsString] {
        &self.arguments
    }

    pub fn stdin(&self) -> &[u8] {
        &self.stdin
    }
}

/// Process observations required by the Toolchain publication policy.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LinkObservation {
    exit_code: Option<i32>,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

impl LinkObservation {
    pub fn new(exit_code: Option<i32>, stdout: Vec<u8>, stderr: Vec<u8>) -> Self {
        Self {
            exit_code,
            stdout,
            stderr,
        }
    }
}

fn execute_link(invocation: &LinkInvocation) -> Result<LinkObservation, ToolchainError> {
    let mut child = Command::new(invocation.program())
        .args(invocation.arguments())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|source| ToolchainError::Start {
            tool: invocation.program().to_owned(),
            source,
        })?;

    let write_result = child
        .stdin
        .take()
        .expect("piped toolchain stdin must be available")
        .write_all(invocation.stdin());
    let result = child
        .wait_with_output()
        .map_err(|source| ToolchainError::Wait {
            tool: invocation.program().to_owned(),
            source,
        })?;
    if let Err(source) = write_result {
        return Err(ToolchainError::WriteAssembly {
            tool: invocation.program().to_owned(),
            source,
        });
    }
    Ok(LinkObservation::new(
        result.status.code(),
        result.stdout,
        result.stderr,
    ))
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
    Execute {
        tool: OsString,
        details: String,
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
            Self::Execute { tool, details } => write!(
                formatter,
                "could not execute toolchain `{}`: {details}",
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
            Self::RuntimeArchiveMissing | Self::Execute { .. } | Self::Failed { .. } => None,
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
