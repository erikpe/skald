use std::{fmt, io, path::PathBuf};

/// A failure to establish or observe a child process boundary.
#[derive(Debug)]
pub struct ProcessError {
    program: PathBuf,
    action: &'static str,
    source: io::Error,
}

impl ProcessError {
    pub(super) fn new(program: PathBuf, action: &'static str, source: io::Error) -> Self {
        Self {
            program,
            action,
            source,
        }
    }

    pub fn program(&self) -> &std::path::Path {
        &self.program
    }

    pub fn action(&self) -> &str {
        self.action
    }
}

impl fmt::Display for ProcessError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "could not {} process {}: {}",
            self.action,
            self.program.display(),
            self.source
        )
    }
}

impl std::error::Error for ProcessError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}
