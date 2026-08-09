//! Runtime-trace activation and metadata planning for Linux x86-64.

mod activation;
mod instrumentation;
mod metadata;
mod names;

pub(super) use activation::Activations;
pub(super) use instrumentation::{emit_pop, emit_push};
pub(super) use metadata::Metadata;

#[cfg(test)]
pub(super) use metadata::escape_path_bytes;

#[cfg(test)]
mod tests;
