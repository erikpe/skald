//! High-level golden execution fixtures assembled from focused support owners.

pub(crate) mod fake_tools;
pub(crate) mod fixture;
mod specs;
mod temporary;

pub(crate) use fixture::Fixture;
pub(crate) use specs::{write_compile_fail_spec, write_native_spec};
