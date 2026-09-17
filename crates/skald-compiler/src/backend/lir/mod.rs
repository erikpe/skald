//! Lowered executable drafts. Construction is not verification or publication.

mod builder;
mod call;
mod graph;
mod model;
mod observable;
mod read;
mod scalar;
mod verify;
use read::DraftChecks;
mod schema;
mod trace;

#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use builder::{BuildError, DraftBuilder};
#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use model::{
    Block, BlockHandle, CallableDraft, Definition, Edge, EdgeOccurrence, Instruction,
    InstructionLocation, LifetimeDisposition, LifetimeMarker, Object, ObjectHandle, ObjectRole,
    Operation, ScalarCheck, ScalarDomainEvidence, Terminator, Value, ValueHandle,
};
#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use scalar::{
    AddressStride, BinaryOperation, Constant, Conversion, DivisionResult, MemoryRepresentation,
    ShiftDirection, UnaryOperation,
};

#[cfg(test)]
mod tests;

#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use call::{Call, CallArgument, CallAttribution, CallTarget};
#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use observable::AddressProvenance;
#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use trace::{TraceAction, TracePlan, TraceSite};

#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use verify::{
    verify_callable, CompletionReceipt, VerificationFailure, VerificationReason, VerifiedCallable,
};
