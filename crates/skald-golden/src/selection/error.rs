use std::{error::Error, fmt};

/// An invalid or empty golden-plan selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionError {
    message: String,
}

impl SelectionError {
    pub(super) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for SelectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for SelectionError {}
