//! Immutable native target facts over checked low-level signatures and resources.
//! Target selection owns virtual instructions; physical emission is separate.
mod abi;
mod resources;
mod selected;

#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use abi::{classify, AbiError, CallArity, ComponentAbi};
#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use resources::{Gpr, NativeResources};

#[cfg(test)]
mod tests;
