//! Compiler library for the stage-0 Skald compiler.
//!
//! Modules correspond to explicit pipeline phases. Durable architecture and
//! phase contracts are documented in `docs/compiler/README.md` and
//! `docs/compiler/PHASES_AND_IR.md`. This is an unpublished
//! repository-internal crate: its public paths support workspace tools,
//! integration tests, and debugging, but are not a version-stable API.

pub mod backend;
pub mod diagnostics;
pub mod driver;
mod dump_format;
pub mod external;
pub mod hir;
mod id_table;
pub mod identity;
pub mod intrinsic;
pub mod lexer;
mod lexical_policy;
pub mod literal;
pub mod mir;
pub mod module;
mod object_path;
pub mod passes;
pub mod resolve;
pub mod source;
pub mod syntax;
mod type_capabilities;
pub mod typeck;

#[cfg(test)]
pub(crate) mod test_support;
