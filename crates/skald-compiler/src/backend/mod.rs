//! Backend contract and target registry.
//!
//! Backends consume verified target-independent MIR and target options. They
//! must not inspect parser AST or type-checker state.

pub mod x86_64_sysv;
