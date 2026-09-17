//! Target payloads expose structural requirements; drafts grant no publication.
mod abi;
mod builder;
mod context;
mod description;
mod graph;
mod resources;
mod storage;

#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use abi::{AbiAreas, AbiBindings};
#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use builder::{SelectedBuildError, SelectedBuilder};
#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use context::SelectionContext;
#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use description::{
    AbiArea, AbiBinding, AbiLocation, Bundle, Constraint, Description, Event, Operand, OperandRole,
    Payload, Phase, Representation, RepresentationKind, Scratch, Tie, Timing,
};
#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use resources::{
    BankId, BankKind, ResourceCatalog, ResourceError, UnitId, ViewId,
};
#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use storage::SelectedDraft;

#[cfg(test)]
mod tests;
