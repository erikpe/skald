//! Real-process compiler double for golden-runner integration tests.

mod support;

use std::{
    env,
    ffi::{OsStr, OsString},
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process, thread,
    time::Duration,
};

fn main() {
    if let Err(error) = run(env::args_os().skip(1).collect()) {
        eprintln!("fake compiler: {error}");
        process::exit(125);
    }
}

fn run(arguments: Vec<OsString>) -> Result<(), String> {
    let activity = support::ActivityGuard::from_environment()?;
    let mode = option(&arguments, "--fake-mode")
        .and_then(OsStr::to_str)
        .unwrap_or("success");
    let output = option(&arguments, "-o")
        .map(PathBuf::from)
        .ok_or_else(|| "missing -o output".to_owned())?;
    if let Some(log) = option(&arguments, "--fake-log") {
        append_arguments(Path::new(log), &arguments)?;
    }
    if let Some(delay) = option(&arguments, "--fake-delay-ms") {
        let milliseconds = delay
            .to_str()
            .ok_or_else(|| "--fake-delay-ms must be UTF-8".to_owned())?
            .parse::<u64>()
            .map_err(display)?;
        thread::sleep(Duration::from_millis(milliseconds));
    }
    if let Some(log) = option(&arguments, "--fake-completion-log") {
        let label = option(&arguments, "--fake-label")
            .and_then(OsStr::to_str)
            .ok_or_else(|| "--fake-completion-log requires --fake-label".to_owned())?;
        let mut output = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log)
            .map_err(display)?;
        writeln!(output, "{label}").map_err(display)?;
    }
    let repeated = output
        .file_name()
        .is_some_and(|name| name == "assembly.repeat.s");

    match mode {
        "success" => write_assembly(&output, ""),
        "unexpected-output" => {
            print!("unexpected compiler stdout");
            eprint!("unexpected compiler stderr");
            write_assembly(&output, "")
        }
        "no-assembly" => Ok(()),
        "nondeterministic-assembly" => {
            write_assembly(&output, if repeated { "# repeat\n" } else { "# first\n" })
        }
        "compile-fail" => {
            eprint!("error[FAKE001]: rejected source\n --> fake.ska:1:1\n");
            drop(activity);
            process::exit(1)
        }
        "compile-fail-streams" => {
            println!("compiler stdout: alpha omega");
            eprint!(
                "error[FAKE001]: first rejected construct\n --> fake.ska:1:1\n\
                 error[FAKE002]: second rejected construct\n --> fake.ska:2:1\n"
            );
            drop(activity);
            process::exit(1)
        }
        "nondeterministic-diagnostic" => {
            eprintln!(
                "error[FAKE001]: {} diagnostic",
                if repeated { "repeat" } else { "first" }
            );
            drop(activity);
            process::exit(1)
        }
        "status-two" => {
            drop(activity);
            process::exit(2)
        }
        _ => Err(format!("unknown fake compiler mode {mode:?}")),
    }
}

fn option<'a>(arguments: &'a [OsString], name: &str) -> Option<&'a OsStr> {
    arguments
        .windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].as_os_str())
}

fn append_arguments(path: &Path, arguments: &[OsString]) -> Result<(), String> {
    let mut output = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(display)?;
    for argument in arguments {
        writeln!(output, "{}", argument.to_string_lossy()).map_err(display)?;
    }
    writeln!(output, "-- invocation --").map_err(display)
}

fn write_assembly(path: &Path, prefix: &str) -> Result<(), String> {
    fs::write(
        path,
        format!("{prefix}.text\n.globl main\n.type main,@function\nmain:\n  mov $0, %eax\n  ret\n"),
    )
    .map_err(display)
}

fn display(error: impl std::fmt::Display) -> String {
    error.to_string()
}
