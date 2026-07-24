//! Target-independent shared-owner storage and lifetime operations.

use std::fmt;

use crate::{
    identity::{ClassId, InitializerId, InterfaceId},
    source::Span,
};

use super::{ids::StorageId, instruction::MirArgument, value::MirPlace};

/// The static object view carried by a non-null shared owner.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MirSharedTarget {
    Obj,
    Class(ClassId),
    Interface(InterfaceId),
}

impl MirSharedTarget {
    pub const fn ty(self) -> super::value::MirType {
        match self {
            Self::Obj => super::value::MirType::Obj,
            Self::Class(class) => super::value::MirType::Class(class),
            Self::Interface(interface) => super::value::MirType::Interface(interface),
        }
    }
}

impl fmt::Display for MirSharedTarget {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Obj => formatter.write_str("Obj"),
            Self::Class(class) => write!(formatter, "class {class}"),
            Self::Interface(interface) => write!(formatter, "interface {interface}"),
        }
    }
}

/// Auditable source of allocation. New allocation forms must add a distinct
/// origin rather than borrowing the `new` provenance.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirSharedAllocationOrigin {
    New,
    /// Reserved malformed/foreign MIR provenance. The verifier rejects it.
    Unspecified,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirSharedAllocate {
    pub allocation: StorageId,
    pub class: ClassId,
    pub origin: MirSharedAllocationOrigin,
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

/// Consumes one live owner and installs it without changing the strong count.
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

/// Consumes a fresh owner and initializes one previously uninitialized field.
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
    pub span: Span,
}
