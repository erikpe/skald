//! Primitive optional storage operations.

use crate::{
    identity::{ClassId, CopyAssignmentId, CopyConstructorId},
    source::Span,
};

use super::{
    MirPlace, MirSelectedCopyOperation, MirSharedTarget, OptionalGuardId, StorageId, ValueId,
};

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirOptionalSharedSource {
    Absent,
    Present(StorageId),
    Copy(MirPlace),
    Move(StorageId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirOptionalSharedInitialize {
    pub destination: MirPlace,
    pub source: MirOptionalSharedSource,
    pub target: MirSharedTarget,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirOptionalSharedAssign {
    pub destination: MirPlace,
    pub source: MirOptionalSharedSource,
    pub target: MirSharedTarget,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirOptionalSharedCleanup {
    pub destination: MirPlace,
    pub target: MirSharedTarget,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirOptionalSharedUnwrap {
    pub source: MirPlace,
    pub destination: StorageId,
    pub target: MirSharedTarget,
    pub span: Span,
}
