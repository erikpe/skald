//! Shared final-MIR adapter. Target selection receives only verified lower products.

mod calls;
mod context;
mod control;
mod data;
mod entry;
mod error;
mod failure;
mod memory;
mod numeric;
mod scalar;
mod trace;
mod worklist;

#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use error::LowerError;
#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use worklist::{lower_next, lower_program};

#[cfg(test)]
mod tests;
