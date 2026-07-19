//! Pipeline orchestration and the implementation-independent CLI contract.
//!
//! This module composes phases, artifact publication, and the host toolchain.
//! Individual compiler phases do not depend on it.

mod cli;
mod pipeline;
mod toolchain;

pub use cli::run_cli;
pub use pipeline::{
    compile_source_to_assembly, AssemblyArtifact, CompilationError, CompilationReport,
};
pub use toolchain::{Toolchain, ToolchainError, C_COMPILER_ENV, RUNTIME_ARCHIVE_ENV};

#[cfg(test)]
use cli::{run_cli_with_context, EXIT_COMPILE_ERROR, EXIT_USAGE, HELP};

#[cfg(test)]
mod tests;
