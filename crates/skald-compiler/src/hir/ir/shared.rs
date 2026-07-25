//! Target-independent shared-owner types, sources, and consuming operations.

use crate::{
    identity::{BindingId, ClassId, InitializerId, InterfaceId},
    source::Span,
};

use super::{HirCallArgument, HirExpression, HirFieldPlace, Type};

/// The static object view carried by a non-null shared owner.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum HirSharedTarget {
    Obj,
    Class(ClassId),
    Interface(InterfaceId),
}

/// A named owner whose value use must create another strong owner.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirSharedPlace {
    Binding {
        binding: BindingId,
        target: HirSharedTarget,
        span: Span,
    },
    Field {
        place: HirFieldPlace,
        target: HirSharedTarget,
        span: Span,
    },
}

impl HirSharedPlace {
    pub fn target(&self) -> HirSharedTarget {
        match self {
            Self::Binding { target, .. } | Self::Field { target, .. } => *target,
        }
    }

    pub const fn span(&self) -> Span {
        match self {
            Self::Binding { span, .. } | Self::Field { span, .. } => *span,
        }
    }
}

/// An expression that already owns its shared result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirSharedProducer {
    Allocation(HirSharedAllocation),
    Call(Box<HirExpression>),
    Cast(Box<HirSharedCast>),
}

impl HirSharedProducer {
    pub fn target(&self) -> HirSharedTarget {
        match self {
            Self::Allocation(allocation) => HirSharedTarget::Class(allocation.class),
            Self::Call(call) => match call.ty {
                Type::Shared(target) => target,
                _ => unreachable!("shared call producer must have a shared result"),
            },
            Self::Cast(cast) => cast.target,
        }
    }

    pub const fn span(&self) -> Span {
        match self {
            Self::Allocation(allocation) => allocation.span,
            Self::Call(call) => call.span,
            Self::Cast(cast) => cast.span,
        }
    }
}

/// An owner-preserving checked view of one existing shared allocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirSharedCast {
    pub source: HirSharedSource,
    pub target: HirSharedTarget,
    pub kind: HirSharedCastKind,
    /// Exact dynamic knowledge retained from a produced allocation. Casts
    /// never change it; named and call-produced owners remain dynamic.
    pub exact_dynamic_class: Option<ClassId>,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirSharedCastKind {
    Static,
    RuntimeTerminate,
}

/// The ownership provenance of a shared value before a consuming boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirSharedSource {
    Place(HirSharedPlace),
    Produced(HirSharedProducer),
}

impl HirSharedSource {
    pub fn target(&self) -> HirSharedTarget {
        match self {
            Self::Place(place) => place.target(),
            Self::Produced(producer) => producer.target(),
        }
    }

    pub const fn span(&self) -> Span {
        match self {
            Self::Place(place) => place.span(),
            Self::Produced(producer) => producer.span(),
        }
    }

    pub const fn transfer(&self) -> HirOwnerTransfer {
        match self {
            Self::Place(_) => HirOwnerTransfer::Copy,
            Self::Produced(_) => HirOwnerTransfer::Adopt,
        }
    }

    /// Exact dynamic knowledge retained by a source whose complete allocation
    /// was produced in this expression. Named places and call results remain
    /// dynamic even when their static shared target names a class.
    pub const fn exact_dynamic_class(&self) -> Option<ClassId> {
        match self {
            Self::Produced(HirSharedProducer::Allocation(allocation)) => Some(allocation.class),
            Self::Produced(HirSharedProducer::Cast(cast)) => cast.exact_dynamic_class,
            Self::Place(_) | Self::Produced(HirSharedProducer::Call(_)) => None,
        }
    }
}

/// The owner operation selected at a consuming value boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirOwnerTransfer {
    Copy,
    Adopt,
}

/// A checked shared value entering storage, an argument, or a result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirSharedTransfer {
    pub source: HirSharedSource,
    pub target: HirSharedTarget,
    pub operation: HirOwnerTransfer,
    pub span: Span,
}

/// Replacement of one live shared owner after its incoming owner is secured.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirSharedAssignment {
    pub destination: BindingId,
    pub value: HirSharedTransfer,
    pub span: Span,
}

/// Ordinary exact-class allocation and its selected initializer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirSharedAllocation {
    pub class: ClassId,
    pub initializer: InitializerId,
    pub arguments: Vec<HirCallArgument>,
    pub span: Span,
}

/// Initialization or replacement of a shared owning field.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirSharedFieldWrite {
    pub place: HirFieldPlace,
    pub value: HirSharedTransfer,
    pub kind: HirSharedFieldWriteKind,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirSharedFieldWriteKind {
    Initialize,
    Assign,
}
