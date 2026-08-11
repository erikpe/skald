//! Destination-directed plans for initializing previously uninitialized storage.

use crate::{
    identity::{ClassId, CopyConstructorId, OptionalTypeId},
    source::Span,
};

use super::{
    HirArrayInitialize, HirClassOptionalSource, HirExpression, HirObjectProducer, HirObjectSource,
    HirOptionalSharedInitialize, HirOptionalSource, HirPrimitiveType, HirSelectedCopyOperation,
    HirSharedTransfer,
};

/// The exact operation selected for one stored value destination.
///
/// These plans deliberately do not contain a concrete place. Locals, fields,
/// arguments, results, and array elements can therefore share compatibility
/// and lifecycle selection without rediscovering source shape below HIR.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirStoredValueInitialization {
    Primitive(HirExpression),
    Class(HirObjectDestinationInitialization),
    OptionalPrimitive {
        source: HirOptionalSource,
        payload: HirPrimitiveType,
    },
    OptionalClass(HirClassOptionalDestinationInitialization),
    Array(HirArrayInitialize),
    Shared(HirSharedTransfer),
    OptionalShared(HirOptionalSharedInitialize),
    Optional(Box<super::HirOptionalValue>),
    /// Copy of one complete immutable wrapper through an exact box owner.
    OptionalBoxPointeeCopy {
        source: super::HirSharedSource,
        optional: OptionalTypeId,
        operation: super::HirOptionalCopyPlan,
        span: Span,
    },
}

impl HirStoredValueInitialization {
    pub const fn span(&self) -> Span {
        match self {
            Self::Primitive(value) => value.span,
            Self::Class(value) => value.span(),
            Self::OptionalPrimitive { source, .. } => source.span(),
            Self::OptionalClass(value) => value.span(),
            Self::Array(value) => value.span,
            Self::Shared(value) => value.span,
            Self::OptionalShared(value) => value.span,
            Self::Optional(value) => value.span,
            Self::OptionalBoxPointeeCopy { span, .. } => *span,
        }
    }
}

/// Class initialization for a new destination.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirObjectDestinationInitialization {
    /// The producer receives the final destination directly.
    Direct {
        producer: HirObjectProducer,
        span: Span,
    },
    /// A materialized or named source is copied into the destination.
    Copy {
        source: HirObjectSource,
        operation: HirSelectedCopyOperation<CopyConstructorId>,
        span: Span,
    },
}

impl HirObjectDestinationInitialization {
    pub const fn span(&self) -> Span {
        match self {
            Self::Direct { span, .. } | Self::Copy { span, .. } => *span,
        }
    }
}

/// Optional-class initialization distinguishes direct payload placement from
/// sources whose present value must be copied conditionally.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirClassOptionalDestinationInitialization {
    Absent {
        class: ClassId,
        span: Span,
    },
    Direct {
        class: ClassId,
        producer: HirObjectProducer,
        span: Span,
    },
    Copy {
        class: ClassId,
        source: HirClassOptionalSource,
        operation: HirSelectedCopyOperation<CopyConstructorId>,
        span: Span,
    },
}

impl HirClassOptionalDestinationInitialization {
    pub const fn span(&self) -> Span {
        match self {
            Self::Absent { span, .. } | Self::Direct { span, .. } | Self::Copy { span, .. } => {
                *span
            }
        }
    }
}
