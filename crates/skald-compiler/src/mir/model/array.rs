//! Target-independent array declarations and semantic operations.
//!
//! This model deliberately contains no descriptor bytes, element stride,
//! header layout, or target ABI facts.

use crate::{
    id_table::DenseIdTable,
    identity::{ArrayTypeId, ClassId, CopyAssignmentId, CopyConstructorId, InitializerId},
    source::Span,
};

use super::{
    declarations::MirSelectedCopyOperation,
    ids::{StorageId, ValueId},
    shared::MirSharedTarget,
    value::{MirPlace, MirType},
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MirArrayTypeTable {
    entries: DenseIdTable<ArrayTypeId, MirArrayType>,
}

impl MirArrayTypeTable {
    pub(crate) fn new(entries: Vec<MirArrayType>) -> Self {
        Self {
            entries: DenseIdTable::new(entries, |entry| entry.id),
        }
    }

    pub fn get(&self, id: ArrayTypeId) -> Option<&MirArrayType> {
        self.entries.get(id, |entry| entry.id)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &MirArrayType> {
        self.entries.iter()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[cfg(test)]
    pub(crate) fn entries_mut_for_test(&mut self) -> &mut [MirArrayType] {
        self.entries.entries_mut_for_test()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirArrayType {
    pub id: ArrayTypeId,
    pub element: MirType,
    pub lifecycle: MirArrayLifecycle,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirArrayLifecycle {
    pub default: Option<MirArrayDefaultElement>,
    pub copy: Option<MirArrayCopyElement>,
    pub assignment: Option<MirArrayAssignElement>,
    pub destruction: MirArrayDestroyElement,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirArrayDefaultElement {
    Primitive,
    OptionalAbsent,
    Class {
        class: ClassId,
        initializer: InitializerId,
    },
    ArrayEmpty(ArrayTypeId),
    SharedClass {
        class: ClassId,
        initializer: InitializerId,
    },
    SharedArrayEmpty(ArrayTypeId),
    /// Allocate a distinct exact optional box containing an absent value.
    SharedOptionalBoxAbsent(crate::identity::OptionalBoxTypeId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirArrayCopyElement {
    Primitive,
    OptionalPrimitive,
    Class {
        class: ClassId,
        operation: MirSelectedCopyOperation<CopyConstructorId>,
    },
    OptionalClass {
        class: ClassId,
        operation: MirSelectedCopyOperation<CopyConstructorId>,
    },
    Array(ArrayTypeId),
    Shared(MirSharedTarget),
    OptionalShared(MirSharedTarget),
    Optional(crate::identity::OptionalTypeId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirArrayAssignElement {
    Primitive,
    OptionalPrimitive,
    Class {
        class: ClassId,
        operation: MirSelectedCopyOperation<CopyAssignmentId>,
    },
    OptionalClass {
        class: ClassId,
        copy_constructor: MirSelectedCopyOperation<CopyConstructorId>,
        copy_assignment: MirSelectedCopyOperation<CopyAssignmentId>,
    },
    Array(ArrayTypeId),
    Shared(MirSharedTarget),
    OptionalShared(MirSharedTarget),
    Optional(crate::identity::OptionalTypeId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirArrayDestroyElement {
    Trivial,
    Class(ClassId),
    OptionalClass(ClassId),
    Array(ArrayTypeId),
    Shared(MirSharedTarget),
    OptionalShared(MirSharedTarget),
    Optional(crate::identity::OptionalTypeId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirArrayOwnership {
    Inline,
    Shared,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirArrayLoopKind {
    Ordinary,
    Indexed { binding: StorageId },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirArrayAnchorKind {
    InlineOwner,
    InlineBacking,
    StableSharedOwner,
    CopiedSharedOwner,
    AdoptedSharedOwner,
    SecuredOptionalSharedOwner,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirArrayPositionKind {
    Element,
    SliceBound,
    /// A `u64` byte-range offset, valid through and including array length.
    RangeOffset,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirArrayBoundary {
    Start,
    End,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirArrayFailure {
    AllocationSize,
    IndexOutOfBounds,
    InvalidSliceBounds,
    SliceLengthMismatch,
}

/// Semantic operations from which target lowering selects concrete layout and
/// code. Repeated element operations live in explicit MIR loop blocks.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirArrayInstruction {
    Allocate {
        backing: StorageId,
        array: ArrayTypeId,
        length: ValueId,
        ownership: MirArrayOwnership,
        failure: MirArrayFailure,
        span: Span,
    },
    /// Allocates unpublished storage for an element-list construction and
    /// establishes its initialized-prefix counter at zero.
    AllocateElements {
        backing: StorageId,
        prefix: StorageId,
        array: ArrayTypeId,
        length: u64,
        ownership: MirArrayOwnership,
        failure: MirArrayFailure,
        span: Span,
    },
    /// Starts a dynamic initialized-prefix protocol after checked allocation.
    BeginIndexed {
        backing: StorageId,
        prefix: StorageId,
        length: StorageId,
        span: Span,
    },
    /// Materializes the proven current prefix as the immutable source `i64`
    /// binding for exactly one element epoch.
    BindIndexed {
        backing: StorageId,
        prefix: StorageId,
        length: StorageId,
        binding: StorageId,
        span: Span,
    },
    /// Initializes the current primitive slot and advances the dynamic prefix.
    InitializeIndexedElement {
        backing: StorageId,
        prefix: StorageId,
        value: ValueId,
        span: Span,
    },
    /// Proves that cleanup for the initialized element finished before the
    /// canonical backedge.
    EndIndexedElement {
        backing: StorageId,
        prefix: StorageId,
        length: StorageId,
        span: Span,
    },
    /// Converts the loop's `prefix == length` exit into publication authority.
    CompleteIndexed {
        backing: StorageId,
        prefix: StorageId,
        length: StorageId,
        span: Span,
    },
    /// Initializes the next primitive element in source order and advances
    /// the initialized prefix by exactly one element.
    InitializeElement {
        backing: StorageId,
        prefix: StorageId,
        position: u64,
        value: ValueId,
        span: Span,
    },
    /// Records that the next lifecycle-bearing element has finished
    /// initialization in its final slot and advances the initialized prefix
    /// by one element.
    CompleteElement {
        backing: StorageId,
        prefix: StorageId,
        position: u64,
        span: Span,
    },
    InitializeNext {
        backing: StorageId,
        index: StorageId,
        operation: MirArrayDefaultElement,
        span: Span,
    },
    CopyNext {
        backing: StorageId,
        source: MirPlace,
        index: StorageId,
        operation: MirArrayCopyElement,
        span: Span,
    },
    Publish {
        backing: StorageId,
        destination: StorageId,
        span: Span,
    },
    PublishShared {
        backing: StorageId,
        destination: StorageId,
        array: ArrayTypeId,
        span: Span,
    },
    Adopt {
        destination: MirPlace,
        source: StorageId,
        array: ArrayTypeId,
        span: Span,
    },
    Replace {
        destination: MirPlace,
        source: StorageId,
        array: ArrayTypeId,
        authorization: Option<super::MirCellWriteAuthorization>,
        final_authorization: Option<super::MirFinalWriteAuthorization>,
        span: Span,
    },
    ElementAssign {
        destination: MirPlace,
        source: MirPlace,
        operation: MirArrayAssignElement,
        span: Span,
    },
    DestroyNext {
        owner: MirPlace,
        index: StorageId,
        operation: MirArrayDestroyElement,
        span: Span,
    },
    Release {
        owner: MirPlace,
        array: ArrayTypeId,
        span: Span,
    },
    AnchorBegin {
        anchor: StorageId,
        owner: MirPlace,
        array: ArrayTypeId,
        kind: MirArrayAnchorKind,
        span: Span,
    },
    AnchorEnd {
        anchor: StorageId,
        span: Span,
    },
    AliasBind {
        alias: StorageId,
        source: MirPlace,
        anchor: StorageId,
        span: Span,
    },
    Normalize {
        destination: StorageId,
        owner: MirPlace,
        index: ValueId,
        array: ArrayTypeId,
        kind: MirArrayPositionKind,
        span: Span,
    },
    /// Materializes an already-unsigned range offset for an array owner.
    Offset {
        destination: StorageId,
        owner: MirPlace,
        offset: ValueId,
        array: ArrayTypeId,
        span: Span,
    },
    Boundary {
        destination: StorageId,
        owner: MirPlace,
        array: ArrayTypeId,
        boundary: MirArrayBoundary,
        span: Span,
    },
    SliceCopy {
        destination: StorageId,
        source: MirPlace,
        start: StorageId,
        end: StorageId,
        array: ArrayTypeId,
        operation: MirArrayCopyElement,
        span: Span,
    },
    SliceLengthCheck {
        destination_start: StorageId,
        destination_end: StorageId,
        source: MirPlace,
        array: ArrayTypeId,
        span: Span,
    },
    SliceBoundsCheck {
        start: StorageId,
        end: StorageId,
        array: ArrayTypeId,
        span: Span,
    },
    SliceAssignNext {
        destination: MirPlace,
        source: MirPlace,
        destination_index: StorageId,
        source_index: StorageId,
        operation: MirArrayAssignElement,
        span: Span,
    },
}

impl MirArrayInstruction {
    pub const fn span(&self) -> Span {
        match self {
            Self::Allocate { span, .. }
            | Self::AllocateElements { span, .. }
            | Self::BeginIndexed { span, .. }
            | Self::BindIndexed { span, .. }
            | Self::InitializeIndexedElement { span, .. }
            | Self::EndIndexedElement { span, .. }
            | Self::CompleteIndexed { span, .. }
            | Self::InitializeElement { span, .. }
            | Self::CompleteElement { span, .. }
            | Self::InitializeNext { span, .. }
            | Self::CopyNext { span, .. }
            | Self::Publish { span, .. }
            | Self::PublishShared { span, .. }
            | Self::Adopt { span, .. }
            | Self::Replace { span, .. }
            | Self::ElementAssign { span, .. }
            | Self::DestroyNext { span, .. }
            | Self::Release { span, .. }
            | Self::AnchorBegin { span, .. }
            | Self::AnchorEnd { span, .. }
            | Self::AliasBind { span, .. }
            | Self::Normalize { span, .. }
            | Self::Offset { span, .. }
            | Self::Boundary { span, .. }
            | Self::SliceCopy { span, .. }
            | Self::SliceLengthCheck { span, .. }
            | Self::SliceBoundsCheck { span, .. }
            | Self::SliceAssignNext { span, .. } => *span,
        }
    }
}
