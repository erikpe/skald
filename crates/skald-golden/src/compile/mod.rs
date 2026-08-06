//! Real-compiler location, invocation, observation, and determinism checks.

mod invoke;
mod locate;
mod model;

pub use locate::{locate_compiler, CompilerLocationError};
pub use model::{
    CompilationExecution, CompilationIssue, CompilationKind, CompilerConfig, CompilerObservation,
    Determinism,
};

pub(crate) use invoke::{compile_build, CompilationPurpose};
