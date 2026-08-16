//! Optional storage operations.

use crate::{
    identity::{ClassId, CopyAssignmentId, CopyConstructorId, OptionalTypeId},
    source::Span,
};

use super::{
    MirCellWriteAuthorization, MirPlace, MirSelectedCopyOperation, MirSharedTarget,
    OptionalGuardId, StorageId, ValueId,
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
    pub authorization: Option<MirCellWriteAuthorization>,
    pub final_authorization: Option<super::MirFinalWriteAuthorization>,
    pub span: Span,
}

/// A source for constructing one tagged owning aggregate optional value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirAggregateOptionalSource {
    Absent,
    /// Reserves an absent outer layer whose payload is about to be initialized
    /// directly; a matching publish completes the value.
    Unpublished,
    Copy(MirPlace),
}

/// Initializes an exact owning aggregate optional destination. Explicitly-present
/// construction is represented as absent initialization, destination-directed
/// payload initialization, and publication.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirAggregateOptionalInitialize {
    pub optional: OptionalTypeId,
    pub destination: MirPlace,
    pub source: MirAggregateOptionalSource,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirAggregateOptionalAssign {
    pub optional: OptionalTypeId,
    pub destination: MirPlace,
    pub source: MirAggregateOptionalSource,
    pub authorization: Option<MirCellWriteAuthorization>,
    pub final_authorization: Option<super::MirFinalWriteAuthorization>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirAggregateOptionalPublish {
    pub optional: OptionalTypeId,
    pub destination: MirPlace,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirAggregateOptionalCleanup {
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
    pub authorization: Option<MirCellWriteAuthorization>,
    pub final_authorization: Option<super::MirFinalWriteAuthorization>,
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
    pub payload: super::MirType,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirOptionalViewEnd {
    pub optional: OptionalTypeId,
    pub guard: OptionalGuardId,
    pub source: MirPlace,
    pub payload: super::MirType,
    pub span: Span,
}

/// One guarded optional layer in a polymorphic object-box allocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirOptionalBoxViewBegin {
    pub box_target: crate::identity::OptionalBoxTypeId,
    pub layer: usize,
    pub guard: OptionalGuardId,
    pub owner: StorageId,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirOptionalBoxViewEnd {
    pub box_target: crate::identity::OptionalBoxTypeId,
    pub layer: usize,
    pub guard: OptionalGuardId,
    pub owner: StorageId,
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
    pub authorization: Option<MirCellWriteAuthorization>,
    pub final_authorization: Option<super::MirFinalWriteAuthorization>,
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
