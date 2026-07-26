//! Typed array identities, lifecycle plans, construction, and provenance.

use crate::{
    id_table::DenseIdTable,
    identity::{ArrayTypeId, ClassId, CopyAssignmentId, CopyConstructorId, InitializerId},
    source::Span,
};

use super::{HirExpression, HirFieldPlace, HirSelectedCopyOperation, HirSharedTarget, Type};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HirArrayTypeTable {
    entries: DenseIdTable<ArrayTypeId, HirArrayType>,
}

impl HirArrayTypeTable {
    pub(crate) fn new(entries: Vec<HirArrayType>) -> Self {
        Self {
            entries: DenseIdTable::new(entries, |entry| entry.id),
        }
    }

    pub fn get(&self, id: ArrayTypeId) -> Option<&HirArrayType> {
        self.entries.get(id, |entry| entry.id)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &HirArrayType> {
        self.entries.iter()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirArrayType {
    pub id: ArrayTypeId,
    pub element: Type,
    pub lifecycle: HirArrayLifecycle,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirArrayLifecycle {
    pub default: Option<HirArrayDefaultElement>,
    pub copy: Option<HirArrayCopyElement>,
    pub assignment: Option<HirArrayAssignElement>,
    pub destruction: HirArrayDestroyElement,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirArrayDefaultElement {
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
pub enum HirArrayCopyElement {
    Primitive,
    OptionalPrimitive,
    Class {
        class: ClassId,
        operation: HirSelectedCopyOperation<CopyConstructorId>,
    },
    OptionalClass {
        class: ClassId,
        operation: HirSelectedCopyOperation<CopyConstructorId>,
    },
    Array(ArrayTypeId),
    Shared(HirSharedTarget),
    OptionalShared(HirSharedTarget),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirArrayAssignElement {
    Primitive,
    OptionalPrimitive,
    Class {
        class: ClassId,
        operation: HirSelectedCopyOperation<CopyAssignmentId>,
    },
    OptionalClass {
        class: ClassId,
        operation: HirSelectedCopyOperation<CopyAssignmentId>,
    },
    Array(ArrayTypeId),
    Shared(HirSharedTarget),
    OptionalShared(HirSharedTarget),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirArrayDestroyElement {
    Trivial,
    Class(ClassId),
    OptionalClass(ClassId),
    Array(ArrayTypeId),
    Shared(HirSharedTarget),
    OptionalShared(HirSharedTarget),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirArrayConstruction {
    pub array: ArrayTypeId,
    pub ownership: HirArrayOwnership,
    pub mode: HirArrayConstructionMode,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirArrayOwnership {
    Inline,
    Shared,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirArrayConstructionMode {
    Empty,
    DefaultLength {
        length: Box<HirExpression>,
        element: HirArrayDefaultElement,
    },
    Copy {
        source: HirArraySource,
        element: HirArrayCopyElement,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirArraySource {
    pub expression: Box<HirExpression>,
    pub provenance: HirArrayProvenance,
    pub array: ArrayTypeId,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirArrayProvenance {
    Named,
    Produced,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirArrayInitialize {
    pub source: HirArraySource,
    pub operation: HirArrayTransfer,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirArrayTransfer {
    DeepCopy(HirArrayCopyElement),
    Adopt,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirArrayFieldInitialize {
    pub place: HirFieldPlace,
    pub value: HirArrayInitialize,
    pub span: Span,
}
