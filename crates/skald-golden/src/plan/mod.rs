//! Immutable expanded build-and-run plans.

mod builder;
mod error;
mod explain;
mod identity;
mod legacy;
mod model;
mod paths;

use std::{ffi::OsString, path::Path};

pub use error::PlanError;
pub use model::{
    PlannedBuild, PlannedLeaf, PlannedLeafKind, PlannedRun, PlannedSpec, PlannedTest, ResolvedArgs,
    ResolvedByteSource, ResolvedCompileExpectation, ResolvedInputFile, ResolvedOutputFile,
    ResolvedRunExpectation, ResolvedStreamExpectation, ResolvedWorkingDirectory, TestPlan,
};

/// Discovers, validates, resolves, and expands every new-format golden spec.
///
/// This operation only reads fixture files. It neither creates artifact
/// directories nor starts external processes.
pub fn build_plan(
    golden_root: impl AsRef<Path>,
    artifact_root: impl AsRef<Path>,
    command_line_compiler_args: &[OsString],
) -> Result<TestPlan, PlanError> {
    let discovered = crate::discovery::discover(golden_root.as_ref().to_owned())?;
    builder::build(
        discovered,
        artifact_root.as_ref(),
        command_line_compiler_args,
    )
}
