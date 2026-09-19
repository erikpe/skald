//! Concrete native drafts. Only independent physical checking can authorize publication.
mod format;
mod model;
mod operands;
mod realize;
mod recipes;
mod transfers;
mod verify;
#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use realize::realize_native;
#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use verify::{
    check_native_physical, Inspection, PhysicalFact, PhysicalReceipt, VerifiedPhysicalCallable,
};
#[cfg(test)]
mod tests;
