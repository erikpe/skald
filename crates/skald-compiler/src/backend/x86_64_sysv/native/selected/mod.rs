//! Virtual x86 scalar instructions, bounded flag bundles and independent legality.
mod calls;
mod context;
mod describe;
mod edit;
mod model;
mod numeric;
mod recipes;
mod requests;
mod select;
mod verify;

#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use context::selection_context;
#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use model::{Instruction, Opcode, Origin, Site, ValueRef};
#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use select::{select, SelectionError};
#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use verify::Verifier;
#[cfg(test)]
mod tests;
