//! Primitive optional storage operations.

use crate::{
    identity::{ClassId, CopyAssignmentId, CopyConstructorId},
    source::Span,
};

use super::{MirPlace, MirSelectedCopyOperation, OptionalGuardId, ValueId};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum MirPrimitiveType {
    I64,
    U64,
    U8,
    F64,
    Bool,
}

impl MirPrimitiveType {
    pub const fn payload_type(self) -> super::MirType {
        match self {
            Self::I64 => super::MirType::I64,
            Self::U64 => super::MirType::U64,
            Self::U8 => super::MirType::U8,
            Self::F64 => super::MirType::F64,
            Self::Bool => super::MirType::Bool,
        }
    }
}

impl std::fmt::Display for MirPrimitiveType {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.payload_type().fmt(formatter)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirOptionalSource {
    Absent,
    Present(ValueId),
    Copy(MirPlace),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirOptionalInitialize {
    pub destination: MirPlace,
    pub source: MirOptionalSource,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirOptionalAssign {
    pub destination: MirPlace,
    pub source: MirOptionalSource,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirPresenceTestKind {
    Some,
    None,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirClassOptionalSource {
    Absent,
    Present(MirPlace),
    Copy(MirPlace),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirClassOptionalInitialize {
    pub destination: MirPlace,
    pub source: MirClassOptionalSource,
    pub class: ClassId,
    pub copy_constructor: Option<MirSelectedCopyOperation<CopyConstructorId>>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirClassOptionalAssign {
    pub destination: MirPlace,
    pub source: MirClassOptionalSource,
    pub class: ClassId,
    pub copy_constructor: Option<MirSelectedCopyOperation<CopyConstructorId>>,
    pub copy_assignment: Option<MirSelectedCopyOperation<CopyAssignmentId>>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirClassOptionalPublish {
    pub destination: MirPlace,
    pub class: ClassId,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirClassOptionalCleanup {
    pub destination: MirPlace,
    pub class: ClassId,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirOptionalViewBegin {
    pub guard: OptionalGuardId,
    pub source: MirPlace,
    pub class: ClassId,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirOptionalViewEnd {
    pub guard: OptionalGuardId,
    pub source: MirPlace,
    pub class: ClassId,
    pub span: Span,
}
