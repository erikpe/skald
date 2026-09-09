use std::{fmt, io, path::PathBuf};

/// A failure to establish or observe a child process boundary.
#[derive(Debug)]
pub struct ProcessError {
    program: PathBuf,
    action: &'static str,
    source: io::Error,
    cleanup_failures: Vec<String>,
}

impl ProcessError {
    pub(super) fn new(program: PathBuf, action: &'static str, source: io::Error) -> Self {
        Self {
            program,
            action,
            source,
            cleanup_failures: Vec::new(),
        }
    }

    pub(super) fn with_cleanup_failures(mut self, failures: Vec<String>) -> Self {
        self.cleanup_failures = failures;
        self
    }

    pub fn program(&self) -> &std::path::Path {
        &self.program
    }

    pub fn action(&self) -> &str {
        self.action
    }

    pub fn cleanup_failures(&self) -> &[String] {
        &self.cleanup_failures
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
        )?;
        if !self.cleanup_failures.is_empty() {
            write!(
                formatter,
                "; cleanup also failed: {}",
                self.cleanup_failures.join("; ")
            )?;
        }
        Ok(())
    }
}

impl std::error::Error for ProcessError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}
