//! Native run sandbox preparation, execution, and observation matching.

mod environment;
mod error;
mod model;
mod sandbox;
mod sequential;
mod suite_model;
mod template;

pub use environment::allowlisted_environment;
pub use error::ExecutionError;
pub use model::{
    ExecutionOptions, OutputFileMismatch, OutputFileObservation, RunExecution, RunMismatch,
    SandboxRetention,
};
pub use sandbox::execute_run;
pub use sequential::execute_sequential;
pub use suite_model::{
    BuildExecution, LeafExecution, LinkExecution, RuntimeExecution, RuntimePreparation,
    SequentialExecution, SequentialOptions, StageStatus,
};

pub(crate) use sandbox::remove_run_sandbox;
