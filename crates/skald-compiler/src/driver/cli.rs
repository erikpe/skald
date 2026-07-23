//! Command-line parsing, source and artifact I/O, and process-level behavior.

use std::{
    ffi::{OsStr, OsString},
    fs,
    io::{self, Write},
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};

use crate::{
    backend::{target_by_name, DEFAULT_TARGET_NAME},
    diagnostics::render_diagnostics,
};

use super::{
    artifact::PendingArtifact, compile_source_to_assembly, CompilationError, Toolchain,
    ToolchainError,
};

pub(super) const HELP: &str = concat!(
    "skac - the Skald compiler\n\n",
    "Usage: skac <input.ska> [-o <output>] [--emit asm] [--target x86_64-sysv]\n\n",
    "Options:\n",
    "  -o <output>       Write the executable or assembly to this path\n",
    "  --emit asm        Emit textual assembly instead of an executable\n",
    "  --target <name>    Select the compilation target (default: x86_64-sysv)\n",
    "  -h, --help         Show this help\n",
    "  --version          Show the compiler version",
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
    if options.input.extension() != Some(OsStr::new("ska")) {
        writeln!(
            stderr,
            "skac: input must use the canonical `.ska` suffix: {}",
            options.input.display()
        )?;
        return Ok(EXIT_USAGE);
    }

    let target = match target_by_name(&options.target) {
        Ok(target) => target,
        Err(error) => {
            writeln!(stderr, "skac: {error}")?;
            return Ok(EXIT_USAGE);
        }
    };
    let source_text = match fs::read_to_string(&options.input) {
        Ok(text) => text,
        Err(error) => {
            writeln!(
                stderr,
                "skac: could not read `{}`: {error}",
                options.input.display()
            )?;
            return Ok(EXIT_IO_ERROR);
        }
    };
    if let Some(output) = &options.output {
        match paths_refer_to_same_file(&options.input, output) {
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
    let diagnostic_path = stable_diagnostic_path(&options.input);
    let artifact = match compile_source_to_assembly(diagnostic_path, source_text, target) {
        Ok(artifact) => artifact,
        Err(CompilationError::Diagnostics(report)) => {
            write!(
                stderr,
                "{}",
                render_diagnostics(&report.sources, &report.diagnostics)
            )?;
            return Ok(EXIT_COMPILE_ERROR);
        }
        Err(CompilationError::Backend(error)) => {
            writeln!(stderr, "skac: internal {error}")?;
            return Ok(EXIT_COMPILE_ERROR);
        }
        Err(CompilationError::HirLowering(error)) => {
            writeln!(stderr, "skac: {error}")?;
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
        .output
        .unwrap_or_else(|| default_output_path(&options.input, options.output_kind));
    let result = match options.output_kind {
        OutputKind::Assembly => publish_assembly(&artifact.assembly, &output),
        OutputKind::Executable => toolchain
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
    let input = fs::metadata(input)?;
    match fs::metadata(output) {
        Ok(output) => Ok(input.dev() == output.dev() && input.ino() == output.ino()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

fn stable_diagnostic_path(input: &Path) -> PathBuf {
    if !input.is_absolute() {
        return input.to_owned();
    }
    std::env::current_dir()
        .ok()
        .and_then(|current| input.strip_prefix(current).ok().map(Path::to_owned))
        .unwrap_or_else(|| input.to_owned())
}

fn default_output_path(input: &Path, output_kind: OutputKind) -> PathBuf {
    match output_kind {
        OutputKind::Assembly => input.with_extension("s"),
        OutputKind::Executable => input.with_extension(""),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OutputKind {
    Executable,
    Assembly,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CompileOptions {
    input: PathBuf,
    output: Option<PathBuf>,
    output_kind: OutputKind,
    target: String,
}

enum Command {
    Help,
    Version,
    Compile(CompileOptions),
}

fn parse_command<I>(args: I) -> Result<Command, String>
where
    I: IntoIterator<Item = OsString>,
{
    let mut args = args.into_iter();
    let _program_name = args.next();
    let mut input = None;
    let mut output = None;
    let mut output_kind = OutputKind::Executable;
    let mut emit_seen = false;
    let mut target = None;

    while let Some(argument) = args.next() {
        match argument.to_str() {
            Some("-h" | "--help") => return Ok(Command::Help),
            Some("--version") => return Ok(Command::Version),
            Some("-o" | "--output") => {
                if output.is_some() {
                    return Err("output option specified more than once".to_owned());
                }
                output = Some(PathBuf::from(
                    args.next()
                        .ok_or_else(|| "expected a path after `-o`".to_owned())?,
                ));
            }
            Some("--emit") => {
                if emit_seen {
                    return Err("emit option specified more than once".to_owned());
                }
                emit_seen = true;
                match args.next().and_then(|value| value.into_string().ok()) {
                    Some(value) if value == "asm" => output_kind = OutputKind::Assembly,
                    Some(value) => {
                        return Err(format!(
                            "unsupported emission kind `{value}`; expected `asm`"
                        ))
                    }
                    None => return Err("expected `asm` after `--emit`".to_owned()),
                }
            }
            Some("--target") => {
                if target.is_some() {
                    return Err("target option specified more than once".to_owned());
                }
                target = Some(
                    args.next()
                        .ok_or_else(|| "expected a target name after `--target`".to_owned())?
                        .into_string()
                        .map_err(|_| "target name must be valid UTF-8".to_owned())?,
                );
            }
            Some(value) if value.starts_with('-') => {
                return Err(format!("unknown option `{value}`"));
            }
            _ if input.is_some() => return Err("more than one input file was provided".to_owned()),
            _ => input = Some(PathBuf::from(argument)),
        }
    }

    let input = input.ok_or_else(|| "no input file was provided".to_owned())?;
    Ok(Command::Compile(CompileOptions {
        input,
        output,
        output_kind,
        target: target.unwrap_or_else(|| DEFAULT_TARGET_NAME.to_owned()),
    }))
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
