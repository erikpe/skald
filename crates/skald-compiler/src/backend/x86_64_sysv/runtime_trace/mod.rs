//! Requested-only runtime-trace metadata planning for Linux x86-64.

mod metadata;
mod names;

pub(super) use metadata::Metadata;

#[cfg(test)]
pub(super) use metadata::escape_path_bytes;

#[cfg(test)]
mod tests;
