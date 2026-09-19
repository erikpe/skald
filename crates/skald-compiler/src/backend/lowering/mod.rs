//! Shared final-MIR lowering. Target selection receives only verified products.

mod array;
mod calls;
mod context;
mod control;
mod data;
mod dispatch;
mod entry;
mod error;
mod failure;
mod generated;
mod generated_array;
mod generated_ownership;
mod lifecycle;
mod memory;
mod numeric;
mod optional;
mod optional_access;
mod optional_aggregate;
mod origin;
mod ownership;
mod place;
mod scalar;
mod trace;
mod worklist;

#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use error::LowerError;
#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use worklist::{lower_next, lower_program, lower_program_with};

#[cfg(test)]
mod tests;
