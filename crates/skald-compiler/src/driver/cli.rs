//! Command-line parsing, source and artifact I/O, and process-level behavior.

use std::{
    ffi::OsString,
    io::{self, Write},
    path::PathBuf,
};

use super::{
    request::{ArtifactKind, EntrySelector},
    Toolchain,
};

mod compile;
mod parse;

use compile::compile;
use parse::{parse_command, Command};

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
    "  --omit-runtime-trace    Omit panic runtime traces from generated code\n",
    "  --mir-optimization <none|default>\n",
    "                          Select the final-MIR optimization profile\n",
    "  --disable-mir-pass <name>\n",
    "                          Disable a named final-MIR pass; repeatable\n",
    "  -v, -q                  Increase or decrease operational report detail\n",
    "  --report-level <level>  Select off, phases, details, or trace reports\n",
    "  --diagnostic-level <l>  Render warning or error diagnostics\n",
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
            let _ = writeln!(stderr, "skac: failed to write command output: {error}");
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
