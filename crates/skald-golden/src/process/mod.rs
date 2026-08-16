//! Bounded subprocess, pipe, timeout, and termination ownership.

mod error;
mod model;
mod runner;

use std::time::Duration;

pub(crate) const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

pub use error::ProcessError;
pub use model::{
    PipeFailure, ProcessCommand, ProcessEnvironment, ProcessObservation, ProcessPipe,
    ProcessTermination,
};
pub use runner::run_process;
