//! Real-process host-linker double for golden-runner integration tests.

use std::{
    env,
    ffi::{OsStr, OsString},
    fs::{self, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process, thread,
    time::Duration,
};

fn main() {
    if let Err(error) = run(env::args_os().skip(1).collect()) {
        eprintln!("fake linker: {error}");
        process::exit(125);
    }
}

fn run(arguments: Vec<OsString>) -> Result<(), String> {
    let output = option(&arguments, "-o")
        .map(PathBuf::from)
        .ok_or_else(|| "missing -o output".to_owned())?;
    let mut assembly = Vec::new();
    io::stdin().read_to_end(&mut assembly).map_err(display)?;
    if let Some(path) = env::var_os("SKALD_FAKE_LINK_ASSEMBLY_LOG") {
        fs::write(path, &assembly).map_err(display)?;
    }
    increment_counter("SKALD_FAKE_LINK_COUNT")?;

    match env::var("SKALD_FAKE_LINK_MODE")
        .as_deref()
        .unwrap_or("success")
    {
        "success" => {
            let executable = env::var_os("SKALD_FAKE_LINK_EXECUTABLE")
                .ok_or_else(|| "missing SKALD_FAKE_LINK_EXECUTABLE".to_owned())?;
            fs::copy(executable, output).map(|_| ()).map_err(display)
        }
        "failure" => {
            eprintln!("fake linker rejected assembly");
            process::exit(9)
        }
        "sleep" => {
            thread::sleep(Duration::from_secs(60));
            Ok(())
        }
        mode => Err(format!("unknown fake linker mode {mode:?}")),
    }
}

fn option<'a>(arguments: &'a [OsString], name: &str) -> Option<&'a OsStr> {
    arguments
        .windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].as_os_str())
}

fn increment_counter(name: &str) -> Result<(), String> {
    let Some(path) = env::var_os(name) else {
        return Ok(());
    };
    let mut output = OpenOptions::new()
        .create(true)
        .append(true)
        .open(Path::new(&path))
        .map_err(display)?;
    output.write_all(b"link\n").map_err(display)
}

fn display(error: impl std::fmt::Display) -> String {
    error.to_string()
}
