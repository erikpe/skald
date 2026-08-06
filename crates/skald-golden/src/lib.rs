//! Spec-driven golden-test runner infrastructure.
//!
//! The crate is repository tooling. Its library owns reusable parsing,
//! planning, execution, and reporting responsibilities; the companion binary
//! remains a thin process entry point.

mod cli;
mod compile;
mod discovery;
mod execute;
mod expectation;
mod plan;
mod process;
mod report;
mod selection;
mod spec;

pub use cli::run_cli;
pub use spec::{
    parse_config, parse_spec, ArgSource, ByteSource, CompileExpectation, CompileFailTest,
    ExitExpectation, InputFile, MatchMode, OutputFileExpectation, RepositoryConfig, Run,
    RunExpectation, RunTest, SchemaVersion, Spec, SpecError, StreamExpectation, Test, TestKind,
    Variant, WorkingDirectory,
};
