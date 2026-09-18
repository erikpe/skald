//! Target payloads expose requirements; shared and target checks own publication.
mod abi;
mod area_layout;
pub(in crate::backend) use area_layout::{AbiAreaLayout, AbiSlotLayout};
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
    BankId, BankKind, ResourceCatalog, ResourceError, ResourceView, UnitId, ViewId,
};
#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use storage::{ObjectRole, SelectedDraft};

#[cfg(test)]
mod tests;

#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use verify::{
    verify_selected, SelectedFailure, SelectedProgramBuilder, SelectedReason, SelectedReceipt,
    TargetVerifier, VerifiedSelectedCallable, VerifiedSelectedProgram,
};

mod edit;
#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use edit::{
    EditablePayload, SelectedEditFailure, SelectedEditor, SelectedRemap,
};

mod inspect;
#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use inspect::{InspectPayload, SelectedFact};
