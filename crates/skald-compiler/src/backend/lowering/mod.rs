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
mod io;
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
mod static_lifecycle;
mod strings;
mod trace;
mod worklist;

pub(in crate::backend) use error::LowerError;
pub(in crate::backend) use worklist::lower_program_with;
#[cfg(test)]
pub(in crate::backend) use worklist::{lower_next, lower_program};

#[cfg(test)]
mod tests;
