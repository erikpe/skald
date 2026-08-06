use std::{fmt, io, path::PathBuf};

/// An error while loading exact fixture bytes or decoding process arguments.
#[derive(Debug)]
pub struct ExpectationError {
    path: PathBuf,
    message: String,
    source: Option<io::Error>,
}

impl ExpectationError {
    pub(super) fn io(path: PathBuf, message: impl Into<String>, source: io::Error) -> Self {
        Self {
            path,
            message: message.into(),
            source: Some(source),
        }
    }

    pub(super) fn invalid(path: PathBuf, message: impl Into<String>) -> Self {
        Self {
            path,
            message: message.into(),
            source: None,
        }
    }

    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for ExpectationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.path.display(), self.message)?;
        if let Some(source) = &self.source {
            write!(formatter, ": {source}")?;
        }
        Ok(())
    }
}

impl std::error::Error for ExpectationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source
            .as_ref()
            .map(|source| source as &(dyn std::error::Error + 'static))
    }
}
