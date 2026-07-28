//! Pipeline orchestration and the implementation-independent CLI contract.
//!
//! This module composes phases, artifact publication, and the host toolchain.
//! Individual compiler phases do not depend on it.
//! The repository contract is documented in
//! `docs/compiler/DRIVER_AND_ARTIFACTS.md`.

mod artifact;
mod cli;
mod pipeline;
mod request;
mod toolchain;

pub use cli::run_cli;
pub use pipeline::{
    compile_request_to_assembly, compile_source_to_assembly, AssemblyArtifact, CompilationError,
    CompilationReport,
};
pub use request::{
    ArtifactKind, ArtifactOptions, CompilationEnvironment, CompilationRequest, EntrySelectionError,
    EntrySelector, StandardLibrarySelection, StandardLibrarySelectionError,
};
pub use toolchain::{
    Toolchain, ToolchainError, C_COMPILER_ENV, RUNTIME_ARCHIVE_ENV, STANDARD_LIBRARY_ROOT_ENV,
};

#[cfg(test)]
use cli::{default_output_path, run_cli_with_context, EXIT_COMPILE_ERROR, EXIT_USAGE, HELP};

#[cfg(test)]
mod tests;
