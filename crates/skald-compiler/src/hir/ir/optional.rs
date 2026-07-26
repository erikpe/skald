//! Typed primitive-optional storage operations.

use crate::{
    identity::{BindingId, ClassId, CopyAssignmentId, CopyConstructorId},
    source::Span,
};

use super::{
    HirAccess, HirExpression, HirFieldPlace, HirObjectSource, HirSelectedCopyOperation,
    HirSharedSource, HirSharedTarget,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum HirPrimitiveType {
    I64,
    U64,
    U8,
    F64,
    Bool,
}

impl HirPrimitiveType {
    pub const fn payload_type(self) -> super::Type {
        match self {
            Self::I64 => super::Type::I64,
            Self::U64 => super::Type::U64,
            Self::U8 => super::Type::U8,
            Self::F64 => super::Type::F64,
            Self::Bool => super::Type::Bool,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::I64 => "i64",
            Self::U64 => "u64",
            Self::U8 => "u8",
            Self::F64 => "f64",
            Self::Bool => "bool",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirOptionalPlace {
    pub storage: HirOptionalStorage,
    pub payload: HirPrimitiveType,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirOptionalStorage {
    Binding(BindingId),
    Field(HirFieldPlace),
    ArrayElement(Box<super::HirArrayElementPlace>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirClassOptionalPlace {
    pub storage: HirOptionalStorage,
    pub class: ClassId,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirOptionalSharedPlace {
    pub storage: HirOptionalStorage,
    pub target: HirSharedTarget,
    pub span: Span,
}

/// A supported inline optional container passed through an alias parameter.
///
/// This is a place category, not a reference type: the binding mode carries
/// the borrow and the variant retains the exact optional container type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirOptionalAliasPlace {
    Primitive(HirOptionalPlace),
    Class(HirClassOptionalPlace),
}

impl HirOptionalAliasPlace {
    pub const fn span(&self) -> Span {
        match self {
            Self::Primitive(place) => place.span,
            Self::Class(place) => place.span,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirOptionalSharedSource {
    Absent { span: Span },
    Present(HirSharedSource),
    Copy(HirOptionalSharedPlace),
    Produced(Box<HirExpression>),
}

impl HirOptionalSharedSource {
    pub const fn span(&self) -> Span {
        match self {
            Self::Absent { span } => *span,
            Self::Present(source) => source.span(),
            Self::Copy(place) => place.span,
            Self::Produced(expression) => expression.span,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirOptionalSharedInitialize {
    pub target: HirSharedTarget,
    pub source: HirOptionalSharedSource,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirOptionalSharedAssignment {
    pub destination: HirOptionalSharedPlace,
    pub source: HirOptionalSharedSource,
    pub kind: HirOptionalWriteKind,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirClassOptionalSource {
    Absent {
        span: Span,
    },
    /// An ordinary exact-class source. Produced objects remain explicit so MIR
    /// can construct directly in a new optional's reserved payload.
    Present(HirObjectSource),
    Copy(HirClassOptionalPlace),
    Produced(Box<HirExpression>),
}

impl HirClassOptionalSource {
    pub const fn span(&self) -> Span {
        match self {
            Self::Absent { span } => *span,
            Self::Present(source) => source.span(),
            Self::Copy(place) => place.span,
            Self::Produced(expression) => expression.span,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirClassOptionalInitialize {
    pub class: ClassId,
    pub source: HirClassOptionalSource,
    /// Required only when a present source must be copied.
    pub copy_constructor: Option<HirSelectedCopyOperation<CopyConstructorId>>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirClassOptionalAssignment {
    pub destination: HirClassOptionalPlace,
    pub source: HirClassOptionalSource,
    /// Present-to-absent transition.
    pub copy_constructor: Option<HirSelectedCopyOperation<CopyConstructorId>>,
    /// Present-to-present transition.
    pub copy_assignment: Option<HirSelectedCopyOperation<CopyAssignmentId>>,
    pub kind: HirOptionalWriteKind,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirOptionalSource {
    Absent { span: Span },
    Present(HirExpression),
    Copy(HirOptionalPlace),
    Produced(Box<HirExpression>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirOptionalOperand {
    Place(HirOptionalPlace),
    Produced(Box<HirExpression>),
    ClassPlace(HirClassOptionalPlace),
    ClassProduced(Box<HirExpression>),
    SharedPlace(HirOptionalSharedPlace),
    SharedProduced(Box<HirExpression>),
}

impl HirOptionalOperand {
    pub const fn span(&self) -> Span {
        match self {
            Self::Place(place) => place.span,
            Self::Produced(expression) => expression.span,
            Self::ClassPlace(place) => place.span,
            Self::ClassProduced(expression) => expression.span,
            Self::SharedPlace(place) => place.span,
            Self::SharedProduced(expression) => expression.span,
        }
    }

    pub const fn payload(&self) -> HirPrimitiveType {
        match self {
            Self::Place(place) => place.payload,
            Self::Produced(expression) => match expression.ty {
                super::Type::OptionalPrimitive(payload) => payload,
                _ => panic!("produced optional operand must have optional type"),
            },
            Self::ClassPlace(_)
            | Self::ClassProduced(_)
            | Self::SharedPlace(_)
            | Self::SharedProduced(_) => {
                panic!("class optional payload access is implemented by checked views")
            }
        }
    }

    pub const fn class(&self) -> ClassId {
        match self {
            Self::ClassPlace(place) => place.class,
            Self::ClassProduced(expression) => match expression.ty {
                super::Type::OptionalClass(class) => class,
                _ => panic!("produced class optional operand must have optional class type"),
            },
            Self::Place(_) | Self::Produced(_) | Self::SharedPlace(_) | Self::SharedProduced(_) => {
                panic!("primitive optional operands have no class payload")
            }
        }
    }

    pub const fn shared_target(&self) -> HirSharedTarget {
        match self {
            Self::SharedPlace(place) => place.target,
            Self::SharedProduced(expression) => match expression.ty {
                super::Type::OptionalShared(target) => target,
                _ => panic!("produced optional owner operand must have optional shared type"),
            },
            Self::Place(_) | Self::Produced(_) | Self::ClassPlace(_) | Self::ClassProduced(_) => {
                panic!("inline optional operands have no shared target")
            }
        }
    }
}

/// One checked, non-owning view of an exact inline-class optional payload.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirCheckedOptionalView {
    pub source: HirOptionalOperand,
    pub access: HirAccess,
    pub span: Span,
}

impl HirOptionalSource {
    pub const fn span(&self) -> Span {
        match self {
            Self::Absent { span } => *span,
            Self::Present(expression) => expression.span,
            Self::Copy(place) => place.span,
            Self::Produced(expression) => expression.span,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirOptionalAssignment {
    pub destination: HirOptionalPlace,
    pub payload: HirPrimitiveType,
    pub source: HirOptionalSource,
    pub kind: HirOptionalWriteKind,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirOptionalWriteKind {
    Initialize,
    Assign,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirPresenceTestKind {
    Some,
    None,
}
