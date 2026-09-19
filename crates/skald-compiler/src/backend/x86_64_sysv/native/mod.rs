//! Immutable native target facts over checked low-level signatures and resources.
//! Target selection owns virtual instructions; physical emission is separate.
mod abi;
mod frame;
mod physical;
mod pilot;
mod placement;
mod resources;
mod selected;

#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use abi::{classify, AbiError, CallArity, ComponentAbi};
#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use frame::plan_native_frame;
#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use physical::{
    check_native_physical, realize_native, Inspection, PhysicalFact, PhysicalProgramBuilder,
    PhysicalReceipt, VerifiedAssembly, VerifiedPhysicalCallable,
};
#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use pilot::compile_native_pilot;
#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use placement::{check_native_placement, place_native_baseline};
#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use resources::{Gpr, NativeResources};

#[cfg(test)]
mod tests;
