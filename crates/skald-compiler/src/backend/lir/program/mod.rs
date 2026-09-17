//! Finalized lowered inventory, without requiring resident predecessor bodies.
mod data;
mod inventory;
mod target;

pub(in crate::backend) use data::{DataDefinition, DataInitializer};
pub(in crate::backend) use inventory::{
    InventoryState, ProgramBuilder, ProgramError, VerifiedProgram,
};
pub(in crate::backend) use target::{TargetCatalog, TargetDeclarations};

#[cfg(test)]
mod tests;
