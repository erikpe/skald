use std::{
    error::Error,
    fmt,
    path::{Path, PathBuf},
};

/// A TOML decoding or semantic schema error with its source and field path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecError {
    spec_path: PathBuf,
    field_path: String,
    message: String,
}

impl SpecError {
    pub(super) fn new(
        spec_path: impl AsRef<Path>,
        field_path: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            spec_path: spec_path.as_ref().to_owned(),
            field_path: field_path.into(),
            message: message.into(),
        }
    }

    /// Returns the path supplied for the parsed spec or configuration.
    pub fn spec_path(&self) -> &Path {
        &self.spec_path
    }

    /// Returns the most specific available schema field path.
    pub fn field_path(&self) -> &str {
        &self.field_path
    }

    /// Returns the human-readable reason the field was rejected.
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for SpecError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}: {}: {}",
            self.spec_path.display(),
            self.field_path,
            self.message
        )
    }
}

impl Error for SpecError {}
