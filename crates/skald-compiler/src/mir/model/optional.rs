//! Primitive optional storage operations.

use crate::{
    identity::{ClassId, CopyAssignmentId, CopyConstructorId, OptionalTypeId},
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

/// A source for copying one exact recursive optional value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirNestedOptionalSource {
    Absent,
    /// Reserves an absent outer layer whose payload is about to be initialized
    /// directly; a matching publish completes the value.
    Unpublished,
    Copy(MirPlace),
}

/// Initializes an exact recursive optional destination. Explicitly-present
/// construction is represented as absent initialization, destination-directed
/// payload initialization, and publication.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirNestedOptionalInitialize {
    pub optional: OptionalTypeId,
    pub destination: MirPlace,
    pub source: MirNestedOptionalSource,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirNestedOptionalAssign {
    pub optional: OptionalTypeId,
    pub destination: MirPlace,
    pub source: MirNestedOptionalSource,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirNestedOptionalPublish {
    pub optional: OptionalTypeId,
    pub destination: MirPlace,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirNestedOptionalCleanup {
    pub optional: OptionalTypeId,
    pub destination: MirPlace,
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
    pub optional: OptionalTypeId,
    pub destination: MirPlace,
    pub source: MirClassOptionalSource,
    pub class: ClassId,
    pub copy_constructor: Option<MirSelectedCopyOperation<CopyConstructorId>>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirClassOptionalAssign {
    pub optional: OptionalTypeId,
    pub destination: MirPlace,
    pub source: MirClassOptionalSource,
    pub class: ClassId,
    pub copy_constructor: Option<MirSelectedCopyOperation<CopyConstructorId>>,
    pub copy_assignment: Option<MirSelectedCopyOperation<CopyAssignmentId>>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirClassOptionalPublish {
    pub optional: OptionalTypeId,
    pub destination: MirPlace,
    pub class: ClassId,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirClassOptionalCleanup {
    pub optional: OptionalTypeId,
    pub destination: MirPlace,
    pub class: ClassId,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirOptionalViewBegin {
    pub optional: OptionalTypeId,
    pub guard: OptionalGuardId,
    pub source: MirPlace,
    pub class: ClassId,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirOptionalViewEnd {
    pub optional: OptionalTypeId,
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
    pub optional: OptionalTypeId,
    pub destination: MirPlace,
    pub source: MirOptionalSharedSource,
    pub target: MirSharedTarget,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirOptionalSharedAssign {
    pub optional: OptionalTypeId,
    pub destination: MirPlace,
    pub source: MirOptionalSharedSource,
    pub target: MirSharedTarget,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirOptionalSharedCleanup {
    pub optional: OptionalTypeId,
    pub destination: MirPlace,
    pub target: MirSharedTarget,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirOptionalSharedUnwrap {
    pub optional: OptionalTypeId,
    pub source: MirPlace,
    pub destination: StorageId,
    pub target: MirSharedTarget,
    pub span: Span,
}
