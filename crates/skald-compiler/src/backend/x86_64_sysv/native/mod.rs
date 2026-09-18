//! Immutable native target facts over checked low-level signatures and resources.
//! This owner has no MIR types and does not select or emit instructions.
mod abi;
mod resources;

#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use abi::{classify, AbiError, CallArity, ComponentAbi};
#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use resources::{Gpr, NativeResources};

#[cfg(test)]
mod tests;
