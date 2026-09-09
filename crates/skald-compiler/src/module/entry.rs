use std::{fmt, path::PathBuf};

use super::ModulePath;

/// The selected source identity from which reachable module loading begins.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EntrySelector {
    File(PathBuf),
    Module(ModulePath),
}

impl EntrySelector {
    /// Resolves the two mutually exclusive entry-option forms.
    ///
    /// Filesystem existence and module-provider lookup are deliberately
    /// deferred to loading.
    pub fn from_options(
        positional_file: Option<PathBuf>,
        logical_module: Option<ModulePath>,
    ) -> Result<Self, EntrySelectionError> {
        match (positional_file, logical_module) {
            (Some(path), None) => Ok(Self::File(path)),
            (None, Some(path)) => Ok(Self::Module(path)),
            (None, None) => Err(EntrySelectionError::Missing),
            (Some(_), Some(_)) => Err(EntrySelectionError::Conflicting),
        }
    }
}

/// Invalid selection of the mutually exclusive file and logical entry forms.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EntrySelectionError {
    Missing,
    Conflicting,
}

impl fmt::Display for EntrySelectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing => {
                formatter.write_str("exactly one file or logical module entry is required")
            }
            Self::Conflicting => {
                formatter.write_str("file and logical module entries are mutually exclusive")
            }
        }
    }
}

impl std::error::Error for EntrySelectionError {}
