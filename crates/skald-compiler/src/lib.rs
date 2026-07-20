//! Compiler library for the stage-0 Skald compiler.
//!
//! Modules correspond to explicit pipeline phases. Phase boundaries and their
//! intended contracts are documented in `docs/REPO_STRUCTURE.md`.

pub mod backend;
pub mod diagnostics;
pub mod driver;
pub mod hir;
pub mod lexer;
pub mod literal;
pub mod mir;
pub mod passes;
pub mod resolve;
pub mod source;
pub mod syntax;
pub mod typeck;
