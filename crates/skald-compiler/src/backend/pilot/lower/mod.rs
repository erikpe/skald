//! Shared final-MIR adapter. Target selection receives only verified lower products.

mod context;
mod control;
mod error;
mod failure;
mod memory;
mod numeric;
mod scalar;
mod worklist;

#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use error::{LowerError, PendingFeature};
#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use worklist::lower_next;

#[cfg(test)]
mod tests;
