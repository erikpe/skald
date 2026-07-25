//! Primitive optional storage operations.

use crate::source::Span;

use super::{StorageId, ValueId};

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirOptionalSource {
    Absent,
    Present(ValueId),
    Copy(StorageId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirOptionalInitialize {
    pub destination: StorageId,
    pub source: MirOptionalSource,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirOptionalAssign {
    pub destination: StorageId,
    pub source: MirOptionalSource,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirPresenceTestKind {
    Some,
    None,
}
