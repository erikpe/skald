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
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirArrayDestroyElement {
    Trivial,
    Class(ClassId),
    OptionalClass(ClassId),
    Array(ArrayTypeId),
    Shared(MirSharedTarget),
    OptionalShared(MirSharedTarget),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirArrayOwnership {
    Inline,
    Shared,
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
    Normalize {
        destination: StorageId,
        owner: MirPlace,
        index: ValueId,
        array: ArrayTypeId,
        kind: MirArrayPositionKind,
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
            | Self::Normalize { span, .. }
            | Self::Boundary { span, .. }
            | Self::SliceCopy { span, .. }
            | Self::SliceLengthCheck { span, .. }
            | Self::SliceBoundsCheck { span, .. }
            | Self::SliceAssignNext { span, .. } => *span,
        }
    }
}
