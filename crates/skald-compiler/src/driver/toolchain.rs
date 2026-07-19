//! Host assembler/linker invocation for executable output.

use std::{
    env,
    ffi::{OsStr, OsString},
    fmt, fs,
    io::{self, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicU64, Ordering},
};

pub const C_COMPILER_ENV: &str = "CC";
pub const RUNTIME_ARCHIVE_ENV: &str = "SKALD_RUNTIME_ARCHIVE";

const DEFAULT_RUNTIME_ARCHIVE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../build/runtime/libskald_runtime.a"
);
static NEXT_TEMPORARY_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Toolchain {
    c_compiler: OsString,
    runtime_archive: PathBuf,
}

impl Toolchain {
    pub fn from_environment() -> Self {
        Self {
            c_compiler: env::var_os(C_COMPILER_ENV).unwrap_or_else(|| OsString::from("cc")),
            runtime_archive: env::var_os(RUNTIME_ARCHIVE_ENV)
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(DEFAULT_RUNTIME_ARCHIVE)),
        }
    }

    pub fn new(c_compiler: impl Into<OsString>, runtime_archive: impl Into<PathBuf>) -> Self {
        Self {
            c_compiler: c_compiler.into(),
            runtime_archive: runtime_archive.into(),
        }
    }

    pub fn link_assembly(&self, assembly: &str, output: &Path) -> Result<(), ToolchainError> {
        if !self.runtime_archive.is_file() {
            return Err(ToolchainError::RuntimeArchiveMissing);
        }

        let temporary = TemporaryOutput::new(output);
        let mut child = Command::new(&self.c_compiler)
            .args([OsStr::new("-x"), OsStr::new("assembler"), OsStr::new("-")])
            .args([OsStr::new("-x"), OsStr::new("none")])
            .arg(&self.runtime_archive)
            .arg("-o")
            .arg(temporary.path())
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

        temporary.publish(output)
    }
}

#[derive(Debug)]
pub enum ToolchainError {
    RuntimeArchiveMissing,
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
            Self::Start { source, .. }
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

struct TemporaryOutput {
    path: PathBuf,
}

impl TemporaryOutput {
    fn new(destination: &Path) -> Self {
        let id = NEXT_TEMPORARY_ID.fetch_add(1, Ordering::Relaxed);
        let mut path = destination.as_os_str().to_os_string();
        path.push(format!(".skac-{}-{id}.tmp", std::process::id()));
        Self {
            path: PathBuf::from(path),
        }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn publish(mut self, destination: &Path) -> Result<(), ToolchainError> {
        fs::rename(&self.path, destination).map_err(|source| ToolchainError::Publish { source })?;
        self.path.clear();
        Ok(())
    }
}

impl Drop for TemporaryOutput {
    fn drop(&mut self) {
        if !self.path.as_os_str().is_empty() {
            let _ = fs::remove_file(&self.path);
        }
    }
}
