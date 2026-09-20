//! Conservative private homes and descriptor-local resource placement.
mod operands;
mod produce;

pub(in crate::backend) use produce::place_baseline;
#[cfg(test)]
pub(in crate::backend) use produce::produce_baseline;
