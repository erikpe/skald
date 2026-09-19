//! Private whole-program native pilot. Public and default emission remain legacy.
mod error;
mod observation;
mod pipeline;

pub(in crate::backend) use error::NativePilotError;
pub(in crate::backend) use observation::NativePilotInspection;
#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use pipeline::{compile_native_pilot, compile_native_pilot_inspected};

#[cfg(test)]
mod tests;
