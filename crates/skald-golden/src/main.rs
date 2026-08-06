use std::process::ExitCode;

fn main() -> ExitCode {
    skald_golden::run_cli(std::env::args_os())
}
