//! Versioned TOML schema and semantic validation.

mod error;
mod model;
mod raw;
mod validation;

use std::path::Path;

pub use error::SpecError;
pub use model::{
    ArgSource, ByteSource, CompileExpectation, CompileFailTest, ExitExpectation, InputFile,
    MatchMode, OutputFileExpectation, RepositoryConfig, Run, RunExpectation, RunTest,
    SchemaVersion, Spec, StreamExpectation, Test, TestKind, Variant, WorkingDirectory,
};
use raw::{RawConfig, RawSpec};

/// Parses and validates one supported golden specification.
pub fn parse_spec(path: impl AsRef<Path>, contents: &str) -> Result<Spec, SpecError> {
    let path = path.as_ref();
    deserialize(path, contents).and_then(|raw: RawSpec| validation::validate_spec(path, raw))
}

/// Parses and validates the repository build-variant configuration.
pub fn parse_config(path: impl AsRef<Path>, contents: &str) -> Result<RepositoryConfig, SpecError> {
    let path = path.as_ref();
    deserialize(path, contents).and_then(|raw: RawConfig| validation::validate_config(path, raw))
}

fn deserialize<'de, T>(path: &Path, contents: &'de str) -> Result<T, SpecError>
where
    T: serde::Deserialize<'de>,
{
    let deserializer = toml::de::Deserializer::parse(contents)
        .map_err(|error| SpecError::new(path, "<document>", error.to_string()))?;

    serde_path_to_error::deserialize(deserializer).map_err(|error| {
        let field = error.path().to_string();
        let field = if field.is_empty() {
            "<document>"
        } else {
            &field
        };
        SpecError::new(path, field, error.inner().to_string())
    })
}
