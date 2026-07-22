//! Compiler library for the stage-0 Skald compiler.
//!
//! Modules correspond to explicit pipeline phases. Phase boundaries and their
//! intended contracts are documented in `docs/REPO_STRUCTURE.md`.

pub mod backend;
pub mod diagnostics;
pub mod driver;
mod dump_format;
mod function_table;
pub mod hir;
pub mod identity;
pub mod lexer;
mod lexical_policy;
pub mod literal;
pub mod mir;
mod object_path;
pub mod passes;
pub mod resolve;
pub mod source;
pub mod syntax;
pub mod typeck;

#[cfg(test)]
pub(crate) mod test_support;
