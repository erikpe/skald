//! Target payloads expose requirements; shared and target checks own publication.
mod abi;
mod builder;
mod context;
mod description;
mod graph;
mod resources;
mod storage;
mod verify;

#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use abi::{AbiAreas, AbiBindings};
#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use builder::{SelectedBuildError, SelectedBuilder};
#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use context::SelectionContext;
#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use description::{
    AbiArea, AbiBinding, AbiLocation, Bundle, Constraint, Description, Event, Flow, Operand,
    OperandRole, Payload, Phase, Representation, RepresentationKind, Scratch, Tie, Timing,
};
#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use resources::{
    BankId, BankKind, ResourceCatalog, ResourceError, UnitId, ViewId,
};
#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use storage::SelectedDraft;

#[cfg(test)]
mod tests;

#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use verify::{
    verify_selected, SelectedFailure, SelectedProgramBuilder, SelectedReason, SelectedReceipt,
    TargetVerifier, VerifiedSelectedCallable, VerifiedSelectedProgram,
};
