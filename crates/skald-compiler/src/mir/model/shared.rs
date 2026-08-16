//! Target-independent shared-owner storage and lifetime operations.

use std::fmt;

use crate::{
    identity::{
        ClassId, InitializerId, InterfaceId, LiteralDataId, OptionalBoxTypeId, OptionalTypeId,
    },
    source::Span,
};

use super::{ids::StorageId, instruction::MirArgument, value::MirPlace};

/// The static object view carried by a non-null shared owner.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MirSharedTarget {
    Obj,
    Class(ClassId),
    Interface(InterfaceId),
    Array(crate::identity::ArrayTypeId),
    OptionalBox(OptionalBoxTypeId),
}

impl MirSharedTarget {
    pub const fn ty(self) -> super::value::MirType {
        match self {
            Self::Obj => super::value::MirType::Obj,
            Self::Class(class) => super::value::MirType::Class(class),
            Self::Interface(interface) => super::value::MirType::Interface(interface),
            Self::Array(array) => super::value::MirType::Array(array),
            Self::OptionalBox(_) => {
                panic!("optional-box pointee types require program metadata")
            }
        }
    }
}

impl fmt::Display for MirSharedTarget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Obj => formatter.write_str("Obj"),
            Self::Class(class) => write!(formatter, "class {class}"),
            Self::Interface(interface) => write!(formatter, "interface {interface}"),
            Self::Array(array) => write!(formatter, "array {array}"),
            Self::OptionalBox(target) => write!(formatter, "optional-box {target}"),
        }
    }
}

/// Auditable source of allocation. New allocation forms must add a distinct
/// origin rather than borrowing the `new` provenance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirSharedAllocationOrigin {
    New,
    OptionalBox,
    /// Reserved malformed/foreign MIR provenance. The verifier rejects it.
    Unspecified,
}

/// Exact physical target allocated behind one unpublished shared handle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirSharedAllocationTarget {
    Class(ClassId),
    OptionalBox {
        target: OptionalBoxTypeId,
        optional: OptionalTypeId,
    },
}

impl MirSharedAllocationTarget {
    pub const fn payload_type(self) -> super::MirType {
        match self {
            Self::Class(class) => super::MirType::Class(class),
            Self::OptionalBox { optional, .. } => super::MirType::Optional(optional),
        }
    }
}

impl fmt::Display for MirSharedAllocationTarget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Class(class) => write!(formatter, "class {class}"),
            Self::OptionalBox { target, optional } => {
                write!(formatter, "optional-box {target} payload={optional}")
            }
        }
    }
}

/// The operation that must establish the unpublished payload before
/// publication. Copy allocation repeats its already-established source here
/// so verification can prove that a check succeeded before allocation and
/// that the same source reaches the selected copy operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirSharedAllocationMode {
    Initialize,
    Copy {
        source: MirPlace,
    },
    OptionalBox {
        completion: MirOptionalBoxCompletion,
    },
}

/// The existing optional instruction that must complete an unpublished box
/// payload before shared publication is legal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirOptionalBoxCompletion {
    OptionalInitialize,
    ClassInitialize,
    ClassPublish,
    OptionalSharedInitialize,
    AggregateInitialize,
    AggregatePublish,
    DestinationCall,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirSharedAllocate {
    pub allocation: StorageId,
    pub target: MirSharedAllocationTarget,
    pub origin: MirSharedAllocationOrigin,
    pub mode: MirSharedAllocationMode,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirSharedInitialize {
    pub allocation: StorageId,
    pub target: InitializerId,
    pub arguments: Vec<MirArgument>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirSharedPublish {
    pub allocation: StorageId,
    pub span: Span,
}

/// Produces one ordinary shared owner for verified immutable static backing.
/// Unlike dynamic allocation, the backing is already complete and published.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirSharedStatic {
    pub destination: StorageId,
    pub data: LiteralDataId,
    pub target: MirSharedTarget,
    pub origin: super::MirStaticAllocationOrigin,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirSharedAdopt {
    pub destination: StorageId,
    pub allocation: StorageId,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirSharedCopy {
    pub destination: StorageId,
    pub source: StorageId,
    pub span: Span,
}

/// Copies one live owner stored in an object field into fresh owner storage.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirSharedFieldCopy {
    pub destination: StorageId,
    pub source: MirPlace,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirSharedCastSource {
    Owner {
        storage: StorageId,
        target: MirSharedTarget,
    },
    Field {
        place: MirPlace,
        target: MirSharedTarget,
    },
}

impl MirSharedCastSource {
    pub const fn target(&self) -> MirSharedTarget {
        match self {
            Self::Owner { target, .. } | Self::Field { target, .. } => *target,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirSharedCastTransfer {
    Copy,
    Adopt,
}

/// An owner-preserving cast. Runtime forms establish `destination` only on the
/// success edge; neither form changes the allocation or its metadata.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirSharedCast {
    pub destination: StorageId,
    pub source: MirSharedCastSource,
    pub target: MirSharedTarget,
    pub transfer: MirSharedCastTransfer,
    pub exact_dynamic_class: Option<ClassId>,
    pub span: Span,
}

/// Consumes one live temporary owner and initializes or replaces a local owner
/// without changing the strong count. Replacement requires the previous owner
/// to have been released first.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirSharedMove {
    pub destination: StorageId,
    pub source: StorageId,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirSharedRelease {
    pub owner: StorageId,
    pub span: Span,
}

/// Consumes a fresh owner and initializes one previously uninitialized owning
/// place: a receiver field or the current unpublished array element-list slot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirSharedFieldInitialize {
    pub destination: MirPlace,
    pub source: StorageId,
    pub span: Span,
}

/// Consumes a secured owner, releases the old field owner, then installs the
/// secured owner without exposing an empty destination.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirSharedFieldReplace {
    pub destination: MirPlace,
    pub source: StorageId,
    pub authorization: Option<super::MirCellWriteAuthorization>,
    pub span: Span,
}
