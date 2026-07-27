//! Logical module paths and request-local module provenance.
//!
//! This facade owns source-independent module vocabulary. Filesystem provider
//! normalization and reachable graph loading will build on these types without
//! making physical paths part of semantic identity.

mod path;
mod provenance;

pub use path::{ModulePath, ModulePathError, ModulePathErrorKind};
pub use provenance::{ModuleProvenance, ModuleSourceLocation};

#[cfg(test)]
mod tests;
