//! Typed object construction, copying, calls, and place operations.

use crate::{
    identity::{
        BindingId, ClassId, CopyAssignmentId, FieldId, FunctionId, InitializerId, MethodId,
    },
    object_path::ObjectPath,
    source::Span,
};

use super::{
    expression::{HirCallArgument, HirExpression},
    HirAccess,
};

pub type HirObjectPath = ObjectPath;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirBaseInitialization {
    pub base: ClassId,
    pub initializer: InitializerId,
    pub arguments: Vec<HirCallArgument>,
    pub span: Span,
}

/// Whether a class supports one copy operation and which implementation was
/// selected. Synthesized capabilities retain their ordered base and field
/// operations so later phases never infer copying from layout.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirCopyCapability<I> {
    User(HirUserCopy<I>),
    Synthesized(HirSynthesizedCopy<I>),
    Unavailable,
}

impl<I: Copy> HirCopyCapability<I> {
    pub const fn selected(&self) -> Option<HirSelectedCopyOperation<I>> {
        match self {
            Self::User(copy) => Some(HirSelectedCopyOperation::User(copy.operation)),
            Self::Synthesized(operation) => {
                Some(HirSelectedCopyOperation::Synthesized(operation.class))
            }
            Self::Unavailable => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirUserCopy<I> {
    pub operation: I,
    pub base: Option<HirBaseCopy<I>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirSynthesizedCopy<I> {
    pub class: ClassId,
    pub base: Option<HirBaseCopy<I>>,
    pub fields: Vec<HirSynthesizedFieldCopy<I>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HirBaseCopy<I> {
    pub base: ClassId,
    pub operation: HirSelectedCopyOperation<I>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirSynthesizedFieldCopy<I> {
    Primitive {
        field: FieldId,
    },
    Class {
        field: FieldId,
        operation: HirSelectedCopyOperation<I>,
    },
}

impl<I> HirSynthesizedFieldCopy<I> {
    pub const fn field(&self) -> FieldId {
        match self {
            Self::Primitive { field } | Self::Class { field, .. } => *field,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirSelectedCopyOperation<I> {
    User(I),
    Synthesized(ClassId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirObjectInitialization {
    pub destination: HirObjectPlace,
    pub producer: HirObjectProducer,
    /// The validated copy operation omitted by permitted constructor elision.
    /// Calls initialize their result destination directly and leave this empty.
    pub elided_copy: Option<HirSelectedCopyOperation<InitializerId>>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirObjectProducer {
    Construct(HirConstruction),
    Call(HirObjectCall),
}

impl HirObjectProducer {
    pub const fn class(&self) -> ClassId {
        match self {
            Self::Construct(construction) => construction.class,
            Self::Call(call) => call.class,
        }
    }

    pub const fn span(&self) -> Span {
        match self {
            Self::Construct(construction) => construction.span,
            Self::Call(call) => call.span,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirObjectCall {
    pub target: HirObjectCallTarget,
    pub arguments: Vec<HirCallArgument>,
    pub class: ClassId,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirObjectCallTarget {
    Direct(FunctionId),
    Method {
        receiver: HirObjectPlace,
        method: MethodId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirCopyConstruction {
    pub destination: HirObjectPlace,
    pub source: HirObjectSource,
    pub operation: HirSelectedCopyOperation<InitializerId>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirObjectSource {
    Place(HirObjectPlace),
    Produced(HirObjectProducer),
}

impl HirObjectSource {
    pub const fn class(&self) -> ClassId {
        match self {
            Self::Place(place) => place.class(),
            Self::Produced(producer) => producer.class(),
        }
    }

    pub const fn span(&self) -> Span {
        match self {
            Self::Place(place) => place.span(),
            Self::Produced(producer) => producer.span(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirConstruction {
    pub class: ClassId,
    pub initializer: InitializerId,
    pub arguments: Vec<HirCallArgument>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirObjectReturn {
    Copy {
        source: HirObjectSource,
        operation: HirSelectedCopyOperation<InitializerId>,
        class: ClassId,
        span: Span,
    },
    /// The frozen return-elision case: construct directly in return storage.
    Construct {
        construction: HirConstruction,
        omitted_copy: HirSelectedCopyOperation<InitializerId>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirFieldAssignment {
    pub place: HirFieldPlace,
    pub value: HirExpression,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirFieldConstruction {
    pub place: HirFieldPlace,
    pub construction: HirConstruction,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirFieldCopyConstruction {
    pub place: HirFieldPlace,
    pub source: HirObjectPlace,
    pub operation: HirSelectedCopyOperation<InitializerId>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirFieldCopyAssignment {
    pub place: HirFieldPlace,
    pub source: HirObjectPlace,
    pub operation: HirSelectedCopyOperation<CopyAssignmentId>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirCopyAssignment {
    pub destination: HirObjectPlace,
    pub source: HirObjectSource,
    pub operation: HirSelectedCopyOperation<CopyAssignmentId>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirObjectPlace {
    pub path: HirObjectPath,
    pub access: HirAccess,
}

impl HirObjectPlace {
    pub const fn root(&self) -> BindingId {
        self.path.root
    }

    pub const fn class(&self) -> ClassId {
        self.path.class
    }

    pub fn projections(&self) -> &[FieldId] {
        &self.path.projections
    }

    pub const fn span(&self) -> Span {
        self.path.span
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirFieldPlace {
    pub receiver: HirObjectPlace,
    pub field: FieldId,
    pub span: Span,
}
