use std::{env, process::ExitCode};

fn main() -> ExitCode {
    let root = env::args_os().nth(1).unwrap_or_else(|| ".".into());
    let diagnostics = match skald_docs_check::check_repository(root) {
        Ok(diagnostics) => diagnostics,
        Err(error) => {
            eprintln!("docs-check: {error}");
            return ExitCode::FAILURE;
        }
    };

    if diagnostics.is_empty() {
        println!("documentation links and indexes are valid");
        ExitCode::SUCCESS
    } else {
        for diagnostic in diagnostics {
            eprintln!("{diagnostic}");
        }
        ExitCode::FAILURE
    }
}
