//! Command-line parsing, source and artifact I/O, and process-level behavior.

use std::{
    ffi::{OsStr, OsString},
    fs,
    io::{self, Write},
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};

use crate::{backend::target_by_name, diagnostics::render_diagnostics};

use super::{
    artifact::PendingArtifact,
    compile_request_to_assembly,
    request::{ArtifactKind, CompilationEnvironment, CompilationRequest, EntrySelector},
    CompilationError, Toolchain, ToolchainError,
};

mod parse;

use parse::{parse_command, Command, CompileOptions};

pub(super) const HELP: &str = concat!(
    "skac - the Skald compiler\n\n",
    "Usage:\n",
    "  skac <input.ska> [options]\n",
    "  skac --entry <module::path> [options]\n\n",
    "Options:\n",
    "  --entry <path>          Select a logical module entry\n",
    "  --module-root <dir>     Add an anonymous module root; repeatable\n",
    "  --stdlib-root <dir>     Replace the installed standard-library root\n",
    "  --no-stdlib             Disable the standard-library root\n",
    "  -o, --output <path>     Write the executable or assembly to this path\n",
    "  --emit asm              Emit textual assembly instead of an executable\n",
    "  --target <name>         Select the target (default: x86_64-sysv)\n",
    "  -h, --help              Show this help\n",
    "  --version               Show the compiler version\n\n",
    "Output defaults:\n",
    "  skac app/main.ska          -> app/main\n",
    "  skac --entry app::main     -> main\n",
    "  Add --emit asm for .s output.",
);
pub(super) const EXIT_COMPILE_ERROR: i32 = 1;
pub(super) const EXIT_USAGE: i32 = 2;
const EXIT_IO_ERROR: i32 = 74;

/// Runs the command-line compiler and returns a process exit code.
pub fn run_cli<I>(args: I) -> i32
where
    I: IntoIterator<Item = OsString>,
{
    let stdout = io::stdout();
    let stderr = io::stderr();
    let mut stdout = stdout.lock();
    let mut stderr = stderr.lock();

    match run_cli_with_context(
        args,
        &mut stdout,
        &mut stderr,
        &Toolchain::from_environment(),
    ) {
        Ok(exit_code) => exit_code,
        Err(error) => {
            eprintln!("skac: failed to write command output: {error}");
            EXIT_IO_ERROR
        }
    }
}

pub(super) fn run_cli_with_context<I, Stdout, Stderr>(
    args: I,
    stdout: &mut Stdout,
    stderr: &mut Stderr,
    toolchain: &Toolchain,
) -> io::Result<i32>
where
    I: IntoIterator<Item = OsString>,
    Stdout: Write,
    Stderr: Write,
{
    match parse_command(args) {
        Ok(Command::Help) => {
            writeln!(stdout, "{HELP}")?;
            Ok(0)
        }
        Ok(Command::Version) => {
            writeln!(stdout, "skac {}", env!("CARGO_PKG_VERSION"))?;
            Ok(0)
        }
        Err(message) => {
            writeln!(stderr, "skac: {message}\n\n{HELP}")?;
            Ok(EXIT_USAGE)
        }
        Ok(Command::Compile(options)) => compile(options, stderr, toolchain),
    }
}

fn compile<Stderr: Write>(
    options: CompileOptions,
    stderr: &mut Stderr,
    toolchain: &Toolchain,
) -> io::Result<i32> {
    if let EntrySelector::File(input) = &options.entry {
        if input.extension() != Some(OsStr::new("ska")) {
            writeln!(
                stderr,
                "skac: input must use the canonical `.ska` suffix: {}",
                input.display()
            )?;
            return Ok(EXIT_USAGE);
        }
    }

    let target = match target_by_name(&options.target) {
        Ok(target) => target,
        Err(error) => {
            writeln!(stderr, "skac: {error}")?;
            return Ok(EXIT_USAGE);
        }
    };
    let working_directory = match std::env::current_dir() {
        Ok(path) => path,
        Err(error) => {
            writeln!(
                stderr,
                "skac: could not determine the working directory: {error}"
            )?;
            return Ok(EXIT_IO_ERROR);
        }
    };

    if let EntrySelector::File(input) = &options.entry {
        if let Some(output) = options.artifact.output() {
            match paths_refer_to_same_file(input, output) {
                Ok(true) => {
                    writeln!(
                        stderr,
                        "skac: output path must not refer to the input source: {}",
                        output.display()
                    )?;
                    return Ok(EXIT_USAGE);
                }
                Ok(false) => {}
                Err(error) => {
                    writeln!(stderr, "skac: could not resolve the output path: {error}")?;
                    return Ok(EXIT_IO_ERROR);
                }
            }
        }
    }

    let request = CompilationRequest::new(
        options.entry.clone(),
        options.module_roots,
        options.standard_library,
        target,
        options.artifact.clone(),
        CompilationEnvironment::new(
            working_directory,
            toolchain.standard_library_root().to_owned(),
        ),
    );
    let artifact = match compile_request_to_assembly(&request) {
        Ok(artifact) => artifact,
        Err(CompilationError::ProviderConfiguration(errors)) => {
            for error in errors {
                writeln!(stderr, "skac: {error}")?;
            }
            return Ok(EXIT_COMPILE_ERROR);
        }
        Err(CompilationError::Diagnostics(report)) => {
            write!(
                stderr,
                "{}",
                render_diagnostics(&report.sources, &report.diagnostics)
            )?;
            return Ok(EXIT_COMPILE_ERROR);
        }
        Err(CompilationError::MirLowering(errors)) => {
            writeln!(stderr, "skac: MIR lowering is unavailable:\n{errors}")?;
            return Ok(EXIT_COMPILE_ERROR);
        }
        Err(CompilationError::Backend(error)) => {
            writeln!(stderr, "skac: internal {error}")?;
            return Ok(EXIT_COMPILE_ERROR);
        }
        Err(CompilationError::MirVerification(errors)) => {
            writeln!(stderr, "skac: internal MIR verification failed:\n{errors}")?;
            return Ok(EXIT_COMPILE_ERROR);
        }
    };

    if !artifact.report.diagnostics.is_empty() {
        write!(
            stderr,
            "{}",
            render_diagnostics(&artifact.report.sources, &artifact.report.diagnostics)
        )?;
    }

    let output = options
        .artifact
        .output()
        .map(Path::to_owned)
        .unwrap_or_else(|| default_output_path(&options.entry, options.artifact.kind()));
    let result = match options.artifact.kind() {
        ArtifactKind::Assembly => publish_assembly(&artifact.assembly, &output),
        ArtifactKind::Executable => toolchain
            .link_assembly(&artifact.assembly, &output)
            .map_err(DriverError::Toolchain),
    };
    if let Err(error) = result {
        writeln!(stderr, "skac: {error}")?;
        return Ok(error.exit_code());
    }

    Ok(0)
}

fn publish_assembly(assembly: &str, output: &Path) -> Result<(), DriverError> {
    let pending = PendingArtifact::new(output).map_err(DriverError::PrepareOutput)?;
    pending
        .write(assembly.as_bytes())
        .map_err(DriverError::WriteOutput)?;
    pending.publish().map_err(DriverError::PublishOutput)
}

fn paths_refer_to_same_file(input: &Path, output: &Path) -> io::Result<bool> {
    let input = match fs::metadata(input) {
        Ok(input) => input,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    match fs::metadata(output) {
        Ok(output) => Ok(input.dev() == output.dev() && input.ino() == output.ino()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

pub(super) fn default_output_path(entry: &EntrySelector, output_kind: ArtifactKind) -> PathBuf {
    let input = match entry {
        EntrySelector::File(path) => path.clone(),
        EntrySelector::Module(path) => PathBuf::from(
            path.components()
                .last()
                .expect("validated module paths are non-empty"),
        ),
    };
    match output_kind {
        ArtifactKind::Assembly => input.with_extension("s"),
        ArtifactKind::Executable => input.with_extension(""),
    }
}

enum DriverError {
    PrepareOutput(io::Error),
    WriteOutput(io::Error),
    PublishOutput(io::Error),
    Toolchain(ToolchainError),
}

impl DriverError {
    const fn exit_code(&self) -> i32 {
        match self {
            Self::PrepareOutput(_) | Self::WriteOutput(_) | Self::PublishOutput(_) => EXIT_IO_ERROR,
            Self::Toolchain(_) => EXIT_COMPILE_ERROR,
        }
    }
}

impl std::fmt::Display for DriverError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PrepareOutput(error) => {
                write!(formatter, "could not prepare assembly output: {error}")
            }
            Self::WriteOutput(error) => {
                write!(formatter, "could not write assembly output: {error}")
            }
            Self::PublishOutput(error) => {
                write!(formatter, "could not publish assembly output: {error}")
            }
            Self::Toolchain(error) => error.fmt(formatter),
        }
    }
}
