use std::{fmt, io, path::PathBuf};

/// A run that could not be prepared, started, or completely observed.
#[derive(Debug)]
pub struct ExecutionError {
    path: Option<PathBuf>,
    sandbox: Option<PathBuf>,
    message: String,
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
}

impl ExecutionError {
    pub(super) fn plain(message: impl Into<String>) -> Self {
        Self {
            path: None,
            sandbox: None,
            message: message.into(),
            source: None,
        }
    }

    pub(super) fn io(path: PathBuf, message: impl Into<String>, source: io::Error) -> Self {
        Self {
            path: Some(path),
            sandbox: None,
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }

    pub(super) fn source(
        message: impl Into<String>,
        source: impl std::error::Error + Send + Sync + 'static,
    ) -> Self {
        Self {
            path: None,
            sandbox: None,
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }

    pub fn path(&self) -> Option<&std::path::Path> {
        self.path.as_deref()
    }

    pub fn sandbox(&self) -> Option<&std::path::Path> {
        self.sandbox.as_deref()
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub(super) fn with_sandbox(mut self, sandbox: PathBuf) -> Self {
        self.sandbox = Some(sandbox);
        self
    }
}

impl fmt::Display for ExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(path) = &self.path {
            write!(formatter, "{}: ", path.display())?;
        }
        write!(formatter, "{}", self.message)?;
        if let Some(source) = &self.source {
            write!(formatter, ": {source}")?;
        }
        if let Some(sandbox) = &self.sandbox {
            write!(formatter, " (sandbox retained at {})", sandbox.display())?;
        }
        Ok(())
    }
}

impl std::error::Error for ExecutionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn std::error::Error + 'static))
    }
}
