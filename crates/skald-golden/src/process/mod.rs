//! Bounded subprocess, pipe, timeout, and termination ownership.

mod error;
mod model;
mod runner;

pub use error::ProcessError;
pub use model::{
    PipeFailure, ProcessCommand, ProcessEnvironment, ProcessObservation, ProcessPipe,
    ProcessTermination,
};
pub use runner::run_process;
