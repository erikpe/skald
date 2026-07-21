//! Validation of finite inline-class containment.

mod graph;

pub(super) use graph::validate_containment;

pub const RECURSIVE_INLINE_CONTAINMENT: &str = "TYP022";

#[cfg(test)]
mod tests;
