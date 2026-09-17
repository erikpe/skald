//! Lowered executable drafts. Construction is not verification or publication.

mod builder;
mod model;
mod scalar;
mod schema;

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
