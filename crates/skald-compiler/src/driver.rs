//! Pipeline orchestration and the implementation-independent CLI contract.
//!
//! This module composes phases, artifact publication, and the host toolchain.
//! Individual compiler phases do not depend on it.

mod pipeline;
mod toolchain;

use std::{
    ffi::{OsStr, OsString},
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

use crate::{
    backend::{target_by_name, DEFAULT_TARGET_NAME},
    diagnostics::render_diagnostics,
};

pub use pipeline::{
    compile_source_to_assembly, AssemblyArtifact, CompilationError, CompilationReport,
};
pub use toolchain::{Toolchain, ToolchainError, C_COMPILER_ENV, RUNTIME_ARCHIVE_ENV};

const HELP: &str = concat!(
    "skac - the Skald compiler\n\n",
    "Usage: skac <input.ska> [-o <output>] [--emit asm] [--target x86_64-sysv]\n\n",
    "Options:\n",
    "  -o <output>       Write the executable or assembly to this path\n",
    "  --emit asm        Emit textual assembly instead of an executable\n",
    "  --target <name>    Select the compilation target (default: x86_64-sysv)\n",
    "  -h, --help         Show this help\n",
    "  --version          Show the compiler version",
);
const EXIT_COMPILE_ERROR: i32 = 1;
const EXIT_USAGE: i32 = 2;
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

fn run_cli_with_context<I, Stdout, Stderr>(
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
        OutputKind::Assembly => fs::write(&output, artifact.assembly).map_err(DriverError::Write),
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
    Write(io::Error),
    Toolchain(ToolchainError),
}

impl DriverError {
    const fn exit_code(&self) -> i32 {
        match self {
            Self::Write(_) => EXIT_IO_ERROR,
            Self::Toolchain(_) => EXIT_COMPILE_ERROR,
        }
    }
}

impl std::fmt::Display for DriverError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Write(error) => write!(formatter, "could not write output: {error}"),
            Self::Toolchain(error) => error.fmt(formatter),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static NEXT_TEST_ID: AtomicU64 = AtomicU64::new(0);

    fn run(args: &[&str]) -> (i32, String, String) {
        run_with_toolchain(args, &Toolchain::new("false", "missing-runtime.a"))
    }

    fn run_with_toolchain(args: &[&str], toolchain: &Toolchain) -> (i32, String, String) {
        let args = args.iter().map(OsString::from);
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let exit_code = run_cli_with_context(args, &mut stdout, &mut stderr, toolchain).unwrap();

        (
            exit_code,
            String::from_utf8(stdout).unwrap(),
            String::from_utf8(stderr).unwrap(),
        )
    }

    fn test_directory(name: &str) -> PathBuf {
        let id = NEXT_TEST_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "skald-driver-test-{}-{id}-{name}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        path
    }

    #[test]
    fn help_and_version_are_available_without_compilation() {
        let (exit_code, stdout, stderr) = run(&["skac", "--help"]);
        assert_eq!(exit_code, 0);
        assert_eq!(stdout, format!("{HELP}\n"));
        assert!(stderr.is_empty());

        let (exit_code, stdout, stderr) = run(&["skac", "--version"]);
        assert_eq!(exit_code, 0);
        assert_eq!(stdout, format!("skac {}\n", env!("CARGO_PKG_VERSION")));
        assert!(stderr.is_empty());
    }

    #[test]
    fn invalid_arguments_are_usage_errors() {
        let (exit_code, stdout, stderr) = run(&["skac"]);
        assert_eq!(exit_code, EXIT_USAGE);
        assert!(stdout.is_empty());
        assert!(stderr.starts_with("skac: no input file was provided\n"));

        let (exit_code, _, stderr) = run(&["skac", "test.ska", "--emit", "object"]);
        assert_eq!(exit_code, EXIT_USAGE);
        assert!(stderr.contains("unsupported emission kind `object`; expected `asm`"));
    }

    #[test]
    fn assembly_mode_runs_the_pipeline_and_writes_only_assembly() {
        let directory = test_directory("assembly");
        let input = directory.join("answer.ska");
        let output = directory.join("answer.s");
        fs::write(&input, "fn main() -> i64 { return 42; }").unwrap();

        let owned = [
            OsString::from("skac"),
            input.clone().into_os_string(),
            OsString::from("--emit"),
            OsString::from("asm"),
            OsString::from("-o"),
            output.clone().into_os_string(),
        ];
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = run_cli_with_context(
            owned,
            &mut stdout,
            &mut stderr,
            &Toolchain::new("false", "missing-runtime.a"),
        )
        .unwrap();

        assert_eq!(status, 0, "{}", String::from_utf8_lossy(&stderr));
        assert!(stdout.is_empty());
        assert!(stderr.is_empty());
        let text = fs::read_to_string(output).unwrap();
        assert!(text.contains(".globl main"));
        assert!(text.contains("movabsq $42, %rax"));
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn source_diagnostics_are_rendered_and_return_compilation_failure() {
        let directory = test_directory("diagnostic");
        let input = directory.join("broken.ska");
        fs::write(&input, "fn main() -> i64 { return nope; }").unwrap();
        let args = [
            OsString::from("skac"),
            input.clone().into_os_string(),
            OsString::from("--emit"),
            OsString::from("asm"),
        ];
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = run_cli_with_context(
            args,
            &mut stdout,
            &mut stderr,
            &Toolchain::new("false", "missing-runtime.a"),
        )
        .unwrap();

        assert_eq!(status, EXIT_COMPILE_ERROR);
        assert!(stdout.is_empty());
        assert!(String::from_utf8(stderr)
            .unwrap()
            .contains("error[RES003]: unknown name `nope`"));
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn linker_failure_is_a_driver_error_not_a_panic() {
        let directory = test_directory("toolchain-failure");
        let input = directory.join("valid.ska");
        let output = directory.join("valid");
        fs::write(&input, "fn main() -> i64 { return 0; }").unwrap();
        let runtime_placeholder = directory.join("runtime.a");
        fs::write(&runtime_placeholder, "placeholder").unwrap();
        let args = [
            OsString::from("skac"),
            input.into_os_string(),
            OsString::from("-o"),
            output.into_os_string(),
        ];
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let status = run_cli_with_context(
            args,
            &mut stdout,
            &mut stderr,
            &Toolchain::new("false", runtime_placeholder),
        )
        .unwrap();

        assert_eq!(status, EXIT_COMPILE_ERROR);
        assert!(stdout.is_empty());
        assert_eq!(
            String::from_utf8(stderr).unwrap(),
            "skac: toolchain `false` failed with exit status 1\n"
        );
        fs::remove_dir_all(directory).unwrap();
    }
}
