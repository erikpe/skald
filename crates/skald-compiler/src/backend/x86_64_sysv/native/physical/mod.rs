//! Concrete native drafts. Only independent physical checking can authorize publication.
mod format;
mod model;
mod operands;
mod program;
mod realize;
mod recipes;
mod transfers;
mod verify;
pub(in crate::backend) use model::RealizeError;
#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use program::{PhysicalProgramBuilder, ProgramError, VerifiedAssembly};
#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use realize::realize_native;
#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use verify::{
    check_native_physical, Inspection, PhysicalError, PhysicalFact, PhysicalReceipt,
    VerifiedPhysicalCallable,
};
#[cfg(test)]
mod tests;
