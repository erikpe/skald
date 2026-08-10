//! Typed array identities, lifecycle plans, construction, and provenance.

use crate::{
    id_table::DenseIdTable,
    identity::{ArrayTypeId, ClassId, CopyAssignmentId, CopyConstructorId, InitializerId},
    source::Span,
};

use super::{
    HirExpression, HirFieldPlace, HirSelectedCopyOperation, HirSharedTarget, HirStaticPlace, Type,
};

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
    Optional(crate::identity::OptionalTypeId),
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
        copy_constructor: HirSelectedCopyOperation<CopyConstructorId>,
        copy_assignment: HirSelectedCopyOperation<CopyAssignmentId>,
    },
    Array(ArrayTypeId),
    Shared(HirSharedTarget),
    OptionalShared(HirSharedTarget),
    Optional(crate::identity::OptionalTypeId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirArrayDestroyElement {
    Trivial,
    Class(ClassId),
    OptionalClass(ClassId),
    Array(ArrayTypeId),
    Shared(HirSharedTarget),
    OptionalShared(HirSharedTarget),
    Optional(crate::identity::OptionalTypeId),
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
    Elements(HirArrayElementList),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirArrayElementList {
    pub left_brace_span: Span,
    pub elements: Vec<HirArrayElementInitialization>,
    pub comma_spans: Vec<Span>,
    pub right_brace_span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirArrayElementInitialization {
    pub element: Type,
    pub value: super::HirStoredValueInitialization,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirArraySource {
    pub receiver: HirArrayReceiver,
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

/// One evaluated array owner used by a projection.
///
/// Keeping the checked owner expression here is intentional: a produced shared
/// owner or optional unwrap must stay alive for the complete projection, while
/// an ordinary named array may only need its unpublished backing allocation
/// anchored.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirArrayReceiver {
    pub source: HirArrayReceiverSource,
    pub array: ArrayTypeId,
    pub access: super::HirAccess,
    pub ownership: HirArrayReceiverOwnership,
    pub anchor: HirArrayAnchor,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirArrayReceiverSource {
    Inline(Box<HirExpression>),
    Shared(Box<super::HirSharedSource>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirArrayReceiverOwnership {
    Inline,
    ExplicitSharedPointee,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirArrayAnchor {
    InlineOwner,
    InlineBacking,
    StableSharedOwner,
    CopiedSharedOwner,
    AdoptedSharedOwner,
    SecuredOptionalSharedOwner,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirArrayEvaluationOrder {
    ReceiverThenIndex,
    ReceiverThenBounds,
    DestinationThenSourceThenReplace,
    DestinationThenSourceThenStore,
    DestinationBoundsThenSourceThenLengthCheckThenCopy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirArrayIndexNormalization {
    SignedFromEndOnce,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirArrayRuntimeFailure {
    IndexOutOfBoundsTerminate,
    InvalidSliceBoundsTerminate,
    SliceLengthMismatchTerminate,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirArrayIndex {
    pub value: Box<HirExpression>,
    pub normalization: HirArrayIndexNormalization,
    pub failure: HirArrayRuntimeFailure,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirArraySliceBounds {
    pub start: Option<Box<HirExpression>>,
    pub end: Option<Box<HirExpression>>,
    pub normalization: HirArrayIndexNormalization,
    pub failure: HirArrayRuntimeFailure,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirArrayElementPlace {
    pub receiver: HirArrayReceiver,
    pub index: HirArrayIndex,
    pub element: Type,
    pub evaluation: HirArrayEvaluationOrder,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirArraySlice {
    pub receiver: HirArrayReceiver,
    pub bounds: HirArraySliceBounds,
    pub array: ArrayTypeId,
    /// Present for copied slice results; absent for a destination-only slice.
    pub element_copy: Option<HirArrayCopyElement>,
    pub evaluation: HirArrayEvaluationOrder,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirArrayLength {
    pub receiver: HirArrayReceiver,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirArrayPlace {
    Binding {
        binding: crate::identity::BindingId,
        array: ArrayTypeId,
        access: super::HirAccess,
        span: Span,
    },
    Field {
        place: HirFieldPlace,
        array: ArrayTypeId,
        access: super::HirAccess,
        span: Span,
    },
    Static {
        place: HirStaticPlace,
        array: ArrayTypeId,
        span: Span,
    },
    Element(Box<HirArrayElementPlace>),
}

impl HirArrayPlace {
    pub fn array(&self) -> ArrayTypeId {
        match self {
            Self::Binding { array, .. }
            | Self::Field { array, .. }
            | Self::Static { array, .. } => *array,
            Self::Element(place) => match place.element {
                Type::Array(array) => array,
                _ => unreachable!("array element place must have an array element type"),
            },
        }
    }

    pub const fn access(&self) -> super::HirAccess {
        match self {
            Self::Binding { access, .. } | Self::Field { access, .. } => *access,
            Self::Static { .. } => super::HirAccess::Mutable,
            Self::Element(place) => place.receiver.access,
        }
    }

    pub const fn span(&self) -> Span {
        match self {
            Self::Binding { span, .. } | Self::Field { span, .. } | Self::Static { span, .. } => {
                *span
            }
            Self::Element(place) => place.span,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirArrayAssignment {
    pub destination: HirArrayPlace,
    pub value: HirArrayInitialize,
    pub evaluation: HirArrayEvaluationOrder,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirArrayElementValue {
    Value(HirExpression),
    Array(HirArrayInitialize),
    Shared(super::HirSharedTransfer),
    OptionalShared(super::HirOptionalSharedInitialize),
    Optional {
        source: super::HirOptionalSource,
        payload: super::HirPrimitiveType,
    },
    ClassOptional(super::HirClassOptionalInitialize),
    NestedOptional(Box<super::HirOptionalValue>),
    Object {
        source: super::HirObjectSource,
        operation: HirSelectedCopyOperation<CopyAssignmentId>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirArrayElementAssignment {
    pub destination: HirArrayElementPlace,
    pub value: HirArrayElementValue,
    pub operation: HirArrayAssignElement,
    pub evaluation: HirArrayEvaluationOrder,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirArraySliceAssignment {
    pub destination: HirArraySlice,
    pub source: HirArraySource,
    pub operation: HirArrayAssignElement,
    pub failure: HirArrayRuntimeFailure,
    pub evaluation: HirArrayEvaluationOrder,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirArrayAliasSource {
    Whole(Box<HirArrayReceiver>),
    Element(Box<HirArrayElementPlace>),
    /// A checked, call-scoped view into an inline optional array payload.
    OptionalPayload {
        source: Box<super::HirOptionalOperand>,
        optional: crate::identity::OptionalTypeId,
        array: ArrayTypeId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirArrayAliasArgument {
    pub source: HirArrayAliasSource,
    pub target: Type,
    pub access: super::HirAccess,
    pub span: Span,
}
