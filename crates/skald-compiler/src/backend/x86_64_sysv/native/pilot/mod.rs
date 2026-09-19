//! Private whole-program native pilot. Public and default emission remain legacy.
mod error;
mod pipeline;

pub(in crate::backend) use error::NativePilotError;
pub(in crate::backend) use pipeline::compile_native_pilot;

#[cfg(test)]
mod tests;
