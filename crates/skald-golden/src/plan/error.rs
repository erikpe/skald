use crate::SpecError;
use std::{error::Error, fmt, path::Path};

/// A deterministic discovery, path-resolution, or plan-expansion failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanError {
    path: Option<std::path::PathBuf>,
    field: Option<String>,
    message: String,
}

impl PlanError {
    pub(crate) fn at_path(path: impl AsRef<Path>, message: impl Into<String>) -> Self {
        Self {
            path: Some(path.as_ref().to_owned()),
            field: None,
            message: message.into(),
        }
    }

    pub(crate) fn at_field(
        path: impl AsRef<Path>,
        field: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            path: Some(path.as_ref().to_owned()),
            field: Some(field.into()),
            message: message.into(),
        }
    }

    pub(crate) fn message(message: impl Into<String>) -> Self {
        Self {
            path: None,
            field: None,
            message: message.into(),
        }
    }

    pub(crate) fn from_spec(error: SpecError) -> Self {
        Self {
            path: Some(error.spec_path().to_owned()),
            field: Some(error.field_path().to_owned()),
            message: error.message().to_owned(),
        }
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn field(&self) -> Option<&str> {
        self.field.as_deref()
    }

    pub fn message_text(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for PlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(path) = &self.path {
            write!(formatter, "{}", path.display())?;
            if let Some(field) = &self.field {
                write!(formatter, ": {field}")?;
            }
            write!(formatter, ": ")?;
        }
        formatter.write_str(&self.message)
    }
}

impl Error for PlanError {}
