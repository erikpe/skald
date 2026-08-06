//! Native run sandbox preparation, execution, and observation matching.

mod environment;
mod error;
mod model;
mod sandbox;
mod scheduler;
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
pub use scheduler::execute_parallel;
pub use sequential::execute_sequential;
pub use suite_model::{
    BuildExecution, LeafExecution, LinkExecution, PlanExecution, RuntimeExecution,
    RuntimePreparation, SchedulerFailure, SchedulerOptions, SequentialExecution, SequentialOptions,
    StageOptions, StageStatus,
};

pub(crate) use sandbox::remove_run_sandbox;
