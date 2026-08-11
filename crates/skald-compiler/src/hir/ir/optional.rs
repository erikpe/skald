//! Typed primitive-optional storage operations.

use crate::{
    identity::{BindingId, ClassId, CopyAssignmentId, CopyConstructorId, OptionalTypeId},
    source::Span,
};

use super::{
    HirAccess, HirExpression, HirFieldPlace, HirObjectSource, HirPrimitiveType,
    HirSelectedCopyOperation, HirSharedSource, HirSharedTarget,
};

/// An immutable published optional wrapper addressed through one shared owner.
///
/// Stable local owners are borrowed directly. Replaceable places and produced
/// owners retain this source so MIR can establish a hidden full-expression
/// anchor before exposing the wrapper.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirOptionalBoxPointee {
    pub source: HirSharedSource,
    pub target: crate::identity::OptionalBoxTypeId,
    pub optional: OptionalTypeId,
    pub span: Span,
}

/// A checked non-owning object view through an immutable optional-box owner.
///
/// Unlike an exact optional place, this retains the static box view separately
/// from the allocation's runtime descriptor. It therefore also represents
/// interface and `Obj` views, which have no owning inline optional type.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirOptionalBoxObjectView {
    pub source: HirSharedSource,
    pub box_target: crate::identity::OptionalBoxTypeId,
    pub target: super::HirViewTarget,
    pub access: HirAccess,
    pub span: Span,
}

/// An outer-layer presence observation that does not require an owning inline
/// optional identity for interface or `Obj` box views.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirOptionalBoxPresence {
    pub source: HirSharedSource,
    pub box_target: crate::identity::OptionalBoxTypeId,
    pub kind: HirPresenceTestKind,
    pub span: Span,
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
    Static(super::HirStaticPlace),
    Field(HirFieldPlace),
    ArrayElement(Box<super::HirArrayElementPlace>),
    SharedPointee(Box<HirOptionalBoxPointee>),
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

/// A place containing one exact canonical optional identity.
///
/// Recursive optional lifecycle uses this identity-based place rather than
/// introducing another payload-specific place family for every nesting depth.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirOptionalValuePlace {
    pub storage: HirOptionalStorage,
    pub optional: OptionalTypeId,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirOptionalValue {
    pub optional: OptionalTypeId,
    pub source: HirOptionalValueSource,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirOptionalValueSource {
    Absent,
    Present(Box<super::HirStoredValueInitialization>),
    Copy(HirOptionalValuePlace),
    Produced(Box<HirExpression>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirAggregateOptionalAssignment {
    pub destination: HirOptionalValuePlace,
    pub value: HirOptionalValue,
    pub kind: HirOptionalWriteKind,
    pub span: Span,
}

/// Checked one-layer extraction of an inline array from its optional wrapper.
///
/// Array payloads are owning values, so the consumer receives an independent
/// deep copy rather than a view into guarded wrapper storage.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirOptionalArrayUnwrap {
    pub source: HirOptionalOperand,
    pub optional: OptionalTypeId,
    pub array: crate::identity::ArrayTypeId,
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
    Nested(HirOptionalValuePlace),
}

impl HirOptionalAliasPlace {
    pub const fn span(&self) -> Span {
        match self {
            Self::Primitive(place) => place.span,
            Self::Class(place) => place.span,
            Self::Nested(place) => place.span,
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
    AggregatePlace(HirOptionalValuePlace),
    AggregateProduced(Box<HirExpression>),
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
            Self::AggregatePlace(place) => place.span,
            Self::AggregateProduced(expression) => expression.span,
        }
    }

    pub fn payload(&self, optional_types: &super::HirOptionalTypeTable) -> HirPrimitiveType {
        match self {
            Self::Place(place) => place.payload,
            Self::Produced(expression) => match expression.ty {
                super::Type::Optional(optional) => match optional_types
                    .get(optional)
                    .expect("optional operand must name typed metadata")
                    .storage
                {
                    super::HirOptionalStorageCategory::Scalar => match optional_types
                        .get(optional)
                        .expect("optional operand must name typed metadata")
                        .payload
                    {
                        super::Type::I64 => HirPrimitiveType::I64,
                        super::Type::U64 => HirPrimitiveType::U64,
                        super::Type::U8 => HirPrimitiveType::U8,
                        super::Type::F64 => HirPrimitiveType::F64,
                        super::Type::Bool => HirPrimitiveType::Bool,
                        _ => panic!("scalar optional metadata must have a primitive payload"),
                    },
                    _ => panic!("produced primitive operand must have scalar optional metadata"),
                },
                _ => panic!("produced optional operand must have optional type"),
            },
            Self::ClassPlace(_)
            | Self::ClassProduced(_)
            | Self::SharedPlace(_)
            | Self::SharedProduced(_)
            | Self::AggregatePlace(_)
            | Self::AggregateProduced(_) => {
                panic!("class optional payload access is implemented by checked views")
            }
        }
    }

    pub fn class(&self, optional_types: &super::HirOptionalTypeTable) -> ClassId {
        match self {
            Self::ClassPlace(place) => place.class,
            Self::ClassProduced(expression) => match expression.ty {
                super::Type::Optional(optional) => match optional_types
                    .get(optional)
                    .expect("optional operand must name typed metadata")
                    .storage
                {
                    super::HirOptionalStorageCategory::InlineClass(class) => class,
                    _ => panic!("produced class operand must have inline-class metadata"),
                },
                _ => panic!("produced class optional operand must have optional class type"),
            },
            Self::Place(_)
            | Self::Produced(_)
            | Self::SharedPlace(_)
            | Self::SharedProduced(_)
            | Self::AggregatePlace(_)
            | Self::AggregateProduced(_) => {
                panic!("primitive optional operands have no class payload")
            }
        }
    }

    pub fn shared_target(&self, optional_types: &super::HirOptionalTypeTable) -> HirSharedTarget {
        match self {
            Self::SharedPlace(place) => place.target,
            Self::SharedProduced(expression) => match expression.ty {
                super::Type::Optional(optional) => match optional_types
                    .get(optional)
                    .expect("optional operand must name typed metadata")
                    .storage
                {
                    super::HirOptionalStorageCategory::SharedOwner(target) => target,
                    _ => panic!("produced shared operand must have shared-owner metadata"),
                },
                _ => panic!("produced optional owner operand must have optional shared type"),
            },
            Self::Place(_)
            | Self::Produced(_)
            | Self::ClassPlace(_)
            | Self::ClassProduced(_)
            | Self::AggregatePlace(_)
            | Self::AggregateProduced(_) => {
                panic!("inline optional operands have no shared target")
            }
        }
    }
}

/// An owning checked extraction of one nested optional payload.
///
/// The outer layer is checked once. Its immediate optional payload is then
/// copied into fresh destination storage selected by the consumer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirNestedOptionalUnwrap {
    pub source: HirOptionalOperand,
    pub optional: OptionalTypeId,
    pub payload: OptionalTypeId,
    pub span: Span,
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
