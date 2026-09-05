//! Seal-local constant analysis infrastructure.
//!
//! CLR1 establishes only the checked-carrier proof boundary. Later roadmap
//! stages add graph construction and solving behind this same private facade.

mod carrier;

#[cfg(test)]
mod tests;
