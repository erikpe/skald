//! Test-only process behaviors used to verify the golden runner boundary.

use std::{
    env,
    ffi::OsString,
    fs,
    io::{self, Read, Write},
    process::{self, Command},
    thread,
    time::Duration,
};

#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;

fn main() {
    if let Err(error) = run(env::args_os().skip(1).collect()) {
        eprintln!("fake process: {error}");
        process::exit(125);
    }
}

fn run(mut arguments: Vec<OsString>) -> Result<(), String> {
    if arguments.is_empty() {
        return Err("missing mode".to_owned());
    }
    let mode = arguments.remove(0);
    match mode.to_str() {
        Some("arguments") => write_arguments(&arguments),
        Some("echo") => echo(),
        Some("large-pipes") => large_pipes(&arguments),
        Some("copy-file") => copy_file(&arguments),
        Some("write-file") => write_file(&arguments),
        Some("write-vary-file") => write_vary_file(&arguments),
        Some("prepare-runtime") => prepare_runtime(&arguments),
        Some("vary") => vary(&arguments),
        Some("cwd") => write_os_value(env::current_dir().map_err(display)?.as_os_str()),
        Some("env") => write_environment(&arguments),
        Some("sleep") => sleep(&arguments),
        Some("signal") => signal(),
        Some("descendant") => descendant(),
        Some("fail") => controlled_failure(),
        _ => Err(format!("unknown mode {mode:?}")),
    }
}

fn write_arguments(arguments: &[OsString]) -> Result<(), String> {
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    for argument in arguments {
        write_os_value_to(&mut stdout, argument)?;
        stdout.write_all(&[0]).map_err(display)?;
    }
    Ok(())
}

fn echo() -> Result<(), String> {
    let mut bytes = Vec::new();
    io::stdin().read_to_end(&mut bytes).map_err(display)?;
    io::stdout().write_all(&bytes).map_err(display)?;
    io::stderr().write_all(&bytes).map_err(display)
}

fn large_pipes(arguments: &[OsString]) -> Result<(), String> {
    let size = required_utf8(arguments, 0, "size")?
        .parse::<usize>()
        .map_err(display)?;
    let mut input = Vec::new();
    io::stdin().read_to_end(&mut input).map_err(display)?;
    if input.len() != size {
        return Err(format!(
            "expected {size} input bytes, found {}",
            input.len()
        ));
    }
    io::stdout().write_all(&vec![b'o'; size]).map_err(display)?;
    io::stderr().write_all(&vec![b'e'; size]).map_err(display)
}

fn write_file(arguments: &[OsString]) -> Result<(), String> {
    let path = arguments
        .first()
        .ok_or_else(|| "missing file path".to_owned())?;
    let mut input = Vec::new();
    io::stdin().read_to_end(&mut input).map_err(display)?;
    fs::write(path, input).map_err(display)
}

fn copy_file(arguments: &[OsString]) -> Result<(), String> {
    let input = arguments
        .first()
        .ok_or_else(|| "missing input file path".to_owned())?;
    let output = arguments
        .get(1)
        .ok_or_else(|| "missing output file path".to_owned())?;
    fs::copy(input, output).map(|_| ()).map_err(display)
}

fn write_vary_file(arguments: &[OsString]) -> Result<(), String> {
    let output = arguments
        .first()
        .ok_or_else(|| "missing output file path".to_owned())?;
    let counter = arguments
        .get(1)
        .ok_or_else(|| "missing counter file path".to_owned())?;
    let value = next_counter(counter)?;
    fs::write(output, value.to_string()).map_err(display)
}

fn prepare_runtime(arguments: &[OsString]) -> Result<(), String> {
    let archive = arguments
        .first()
        .ok_or_else(|| "missing runtime archive path".to_owned())?;
    let counter = arguments
        .get(1)
        .ok_or_else(|| "missing runtime counter path".to_owned())?;
    let _ = next_counter(counter)?;
    fs::write(archive, b"!<arch>\n").map_err(display)
}

fn vary(arguments: &[OsString]) -> Result<(), String> {
    let counter = arguments
        .first()
        .ok_or_else(|| "missing counter file path".to_owned())?;
    let value = next_counter(counter)?;
    write!(io::stdout(), "{value}").map_err(display)
}

fn next_counter(path: &std::ffi::OsStr) -> Result<u64, String> {
    let value = fs::read_to_string(path)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(0)
        + 1;
    fs::write(path, value.to_string()).map_err(display)?;
    Ok(value)
}

fn write_environment(arguments: &[OsString]) -> Result<(), String> {
    let name = arguments
        .first()
        .ok_or_else(|| "missing environment name".to_owned())?;
    match env::var_os(name) {
        Some(value) => write_os_value(&value),
        None => io::stdout().write_all(b"<unset>").map_err(display),
    }
}

fn sleep(arguments: &[OsString]) -> Result<(), String> {
    let milliseconds = required_utf8(arguments, 0, "milliseconds")?
        .parse::<u64>()
        .map_err(display)?;
    thread::sleep(Duration::from_millis(milliseconds));
    Ok(())
}

#[cfg(unix)]
fn signal() -> Result<(), String> {
    nix::sys::signal::raise(nix::sys::signal::Signal::SIGTERM).map_err(display)
}

#[cfg(not(unix))]
fn signal() -> Result<(), String> {
    Err("signals are available only on Unix".to_owned())
}

fn descendant() -> Result<(), String> {
    let child = Command::new(env::current_exe().map_err(display)?)
        .args(["sleep", "60000"])
        .spawn()
        .map_err(display)?;
    writeln!(io::stdout(), "{}", child.id()).map_err(display)?;
    io::stdout().flush().map_err(display)?;
    thread::sleep(Duration::from_secs(60));
    Ok(())
}

fn controlled_failure() -> Result<(), String> {
    io::stdout()
        .write_all(b"failure stdout\0\xff")
        .map_err(display)?;
    io::stderr()
        .write_all(b"failure stderr\n")
        .map_err(display)?;
    process::exit(17)
}

fn required_utf8<'a>(
    arguments: &'a [OsString],
    index: usize,
    name: &str,
) -> Result<&'a str, String> {
    arguments
        .get(index)
        .ok_or_else(|| format!("missing {name}"))?
        .to_str()
        .ok_or_else(|| format!("{name} is not UTF-8"))
}

fn write_os_value(value: &std::ffi::OsStr) -> Result<(), String> {
    write_os_value_to(&mut io::stdout(), value)
}

#[cfg(unix)]
fn write_os_value_to(output: &mut impl Write, value: &std::ffi::OsStr) -> Result<(), String> {
    output.write_all(value.as_bytes()).map_err(display)
}

#[cfg(not(unix))]
fn write_os_value_to(output: &mut impl Write, value: &std::ffi::OsStr) -> Result<(), String> {
    output
        .write_all(value.to_string_lossy().as_bytes())
        .map_err(display)
}

fn display(error: impl std::fmt::Display) -> String {
    error.to_string()
}
