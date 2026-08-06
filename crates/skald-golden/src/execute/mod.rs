//! Native run sandbox preparation, execution, and observation matching.

mod environment;
mod error;
mod model;
mod sandbox;
mod template;

pub use environment::allowlisted_environment;
pub use error::ExecutionError;
pub use model::{
    ExecutionOptions, OutputFileMismatch, OutputFileObservation, RunExecution, RunMismatch,
    SandboxRetention,
};
pub use sandbox::execute_run;
