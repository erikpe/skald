//! Pipeline orchestration and the implementation-independent CLI contract.
//!
//! This module may compose phases, but phase implementations must not depend
//! on the driver.

use std::ffi::OsString;

const HELP: &str = "skac - the Skald compiler\n\nUsage: skac <input.ska> [-o <output>] [--emit asm]\n\nThe first compiler slice is not implemented yet.";

/// Runs the current command-line scaffold and returns a process exit code.
pub fn run_cli<I>(args: I) -> i32
where
    I: IntoIterator<Item = OsString>,
{
    let mut args = args.into_iter();
    let _program_name = args.next();

    match args.next().as_deref() {
        Some(arg) if arg == "--help" || arg == "-h" => {
            println!("{HELP}");
            0
        }
        Some(arg) if arg == "--version" => {
            println!("skac {}", env!("CARGO_PKG_VERSION"));
            0
        }
        Some(_) => {
            eprintln!("skac: the first vertical compiler slice is not implemented yet");
            2
        }
        None => {
            eprintln!("{HELP}");
            2
        }
    }
}
