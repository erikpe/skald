//! Typed object construction, copying, calls, and place operations.

use crate::{
    identity::{
        BindingId, ClassId, CopyAssignmentId, CopyConstructorId, FieldId, FunctionId,
        InitializerId, InterfaceId,
    },
    object_path::ObjectPath,
    source::Span,
};

use super::{
    expression::{
        HirCallArgument, HirExpression, HirInterfaceCallTarget, HirInterfaceReceiver,
        HirMethodCallTarget,
    },
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
    OptionalPrimitive {
        field: FieldId,
        payload: super::HirPrimitiveType,
    },
    OptionalClass {
        field: FieldId,
        class: ClassId,
        operation: HirSelectedCopyOperation<I>,
    },
    Shared {
        field: FieldId,
    },
    OptionalShared {
        field: FieldId,
        target: super::HirSharedTarget,
    },
    Class {
        field: FieldId,
        operation: HirSelectedCopyOperation<I>,
    },
    Array {
        field: FieldId,
        array: crate::identity::ArrayTypeId,
    },
}

impl<I> HirSynthesizedFieldCopy<I> {
    pub const fn field(&self) -> FieldId {
        match self {
            Self::Primitive { field }
            | Self::OptionalPrimitive { field, .. }
            | Self::OptionalClass { field, .. }
            | Self::Shared { field }
            | Self::OptionalShared { field, .. }
            | Self::Class { field, .. }
            | Self::Array { field, .. } => *field,
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
    pub elided_copy: Option<HirSelectedCopyOperation<CopyConstructorId>>,
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
        receiver: HirMethodReceiver,
        target: HirMethodCallTarget,
    },
    Interface {
        receiver: HirInterfaceReceiver,
        target: HirInterfaceCallTarget,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirCopyConstruction {
    pub destination: HirObjectPlace,
    pub source: HirObjectSource,
    pub operation: HirSelectedCopyOperation<CopyConstructorId>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirObjectSource {
    Place(HirObjectPlace),
    ArrayElement(Box<super::HirArrayElementPlace>),
    Produced(HirObjectProducer),
    /// A checked, full-expression-bounded class place consumed by an owning
    /// copy operation after its static or runtime selection succeeds.
    Checked(Box<HirCheckedObjectView>),
    Slice(HirObjectSlice),
}

impl HirObjectSource {
    pub const fn class(&self) -> ClassId {
        match self {
            Self::Place(place) => place.class(),
            Self::ArrayElement(place) => match place.element {
                super::Type::Class(class) => class,
                _ => panic!("object array-element source must have a class type"),
            },
            Self::Produced(producer) => producer.class(),
            Self::Checked(view) => match view.class {
                Some(class) => class,
                None => panic!("owning checked sources must select a class"),
            },
            Self::Slice(slice) => slice.target,
        }
    }

    pub const fn span(&self) -> Span {
        match self {
            Self::Place(place) => place.span(),
            Self::ArrayElement(place) => place.span,
            Self::Produced(producer) => producer.span(),
            Self::Checked(view) => view.span,
            Self::Slice(slice) => slice.span,
        }
    }
}

/// An owning conversion that copies one selected ancestor subobject into an
/// independent exact-class destination.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirObjectSlice {
    pub source: Box<HirObjectSource>,
    /// Direct-base identities from the source class to `target`.
    pub bases: Vec<ClassId>,
    pub target: ClassId,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirConstruction {
    pub class: ClassId,
    pub mode: HirConstructionMode,
    pub span: Span,
}

impl HirConstruction {
    pub fn initializer(&self) -> Option<InitializerId> {
        match self.mode {
            HirConstructionMode::Initialize { initializer, .. } => Some(initializer),
            HirConstructionMode::Copy { .. } => None,
        }
    }

    pub fn arguments(&self) -> Option<&[HirCallArgument]> {
        match &self.mode {
            HirConstructionMode::Initialize { arguments, .. } => Some(arguments),
            HirConstructionMode::Copy { .. } => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirConstructionMode {
    Initialize {
        initializer: InitializerId,
        arguments: Vec<HirCallArgument>,
    },
    Copy {
        source: Box<HirObjectSource>,
        operation: HirSelectedCopyOperation<CopyConstructorId>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirObjectReturn {
    Copy {
        source: Box<HirObjectSource>,
        operation: HirSelectedCopyOperation<CopyConstructorId>,
        class: ClassId,
        span: Span,
    },
    /// The supported return-elision case: construct directly in return storage.
    Construct {
        construction: HirConstruction,
        omitted_copy: Option<HirSelectedCopyOperation<CopyConstructorId>>,
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
    pub source: HirObjectSource,
    pub operation: HirSelectedCopyOperation<CopyConstructorId>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirFieldCopyAssignment {
    pub place: HirFieldPlace,
    pub source: HirObjectSource,
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

    pub fn projections(&self) -> &[crate::object_path::ObjectProjection] {
        &self.path.projections
    }

    pub const fn span(&self) -> Span {
        self.path.span
    }
}

/// The complete-object and dynamic-class provenance retained across a
/// non-owning receiver boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirObjectOrigin {
    /// An independently owned inline object whose dynamic class is exact.
    Exact {
        complete: HirObjectPlace,
        dynamic_class: ClassId,
    },
    /// A call-scoped alias or method receiver carrying runtime complete-object
    /// and dynamic-class metadata from its caller.
    Forwarded {
        binding: BindingId,
        static_target: HirViewTarget,
        access: HirAccess,
        /// Restricts virtual selection while a destructor body runs.
        dispatch_limit: Option<ClassId>,
        span: Span,
    },
    /// A stable shared owner whose allocation header supplies the complete
    /// payload address and dynamic metadata for a borrowed pointee view.
    Shared {
        binding: BindingId,
        static_target: HirViewTarget,
        access: HirAccess,
        span: Span,
    },
    /// Provenance placeholder for a call-scoped hidden shared owner. MIR
    /// lowering binds it to the concrete anchor storage created for the view.
    AnchoredShared {
        static_target: HirViewTarget,
        access: HirAccess,
        span: Span,
    },
    /// An exact object produced into a compiler-owned full-expression
    /// temporary. MIR lowering replaces this marker with the temporary place.
    Produced { dynamic_class: ClassId, span: Span },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirMethodReceiver {
    /// The statically selected subobject used for access and direct calls.
    pub place: HirObjectPlace,
    /// The complete object used by virtual selection and nested forwarding.
    pub origin: Box<HirObjectOrigin>,
    /// Present when the receiver place is defined by a full-expression checked
    /// cast rather than by an ordinary stable binding path.
    pub checked_cast: Option<Box<HirCheckedObjectView>>,
    /// Present when receiver evaluation first materializes a hidden strong
    /// owner for a shared field or produced shared value.
    pub shared_view: Option<Box<HirObjectView>>,
    /// Present when receiver evaluation unwraps an inline-class optional.
    pub optional_view: Option<Box<HirObjectView>>,
    /// Present when the receiver is rooted in one checked array element.
    pub array_element: Option<Box<super::HirArrayElementPlace>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirViewTarget {
    Class(ClassId),
    Interface(InterfaceId),
    Obj,
}

/// A non-owning, access-preserving object view used by calls, casts, and type
/// operations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirObjectView {
    pub source: HirViewSource,
    pub origin: Box<HirObjectOrigin>,
    pub target: HirViewTarget,
    pub access: HirAccess,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirCheckedObjectView {
    pub view: HirObjectView,
    /// The view passed to the direct consumer after the cast succeeds. This
    /// may be an implicit up-view of the checked target.
    pub consumer_target: HirViewTarget,
    pub consumer_access: HirAccess,
    pub kind: HirCheckedObjectViewKind,
    /// Projections selected after the cast target (for inherited members and
    /// fields reached through the cast place).
    pub projections: Vec<crate::object_path::ObjectProjection>,
    pub class: Option<ClassId>,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirCheckedObjectViewKind {
    Static,
    RuntimeTerminate,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirViewSource {
    Place(HirObjectPlace),
    Produced(Box<HirObjectProducer>),
    Forwarded {
        binding: BindingId,
        target: HirViewTarget,
        access: HirAccess,
        span: Span,
    },
    /// The complete payload addressed through a stable shared owner.
    Shared {
        binding: BindingId,
        target: HirViewTarget,
        access: HirAccess,
        projections: Vec<crate::object_path::ObjectProjection>,
        span: Span,
    },
    /// A shared owner that must be materialized into hidden call-scoped
    /// storage before exposing its pointee. Places are retained and produced
    /// owners are adopted; both remain live through the full expression.
    AnchoredShared {
        source: Box<super::HirSharedSource>,
        target: HirViewTarget,
        access: HirAccess,
        projections: Vec<crate::object_path::ObjectProjection>,
        span: Span,
    },
    /// A checked exact-class payload whose presence is pinned for the
    /// immediate consumer.
    OptionalPayload {
        view: Box<super::HirCheckedOptionalView>,
        projections: Vec<crate::object_path::ObjectProjection>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirFieldPlace {
    pub receiver: HirObjectPlace,
    pub checked_cast: Option<Box<HirCheckedObjectView>>,
    pub shared_view: Option<Box<HirObjectView>>,
    pub optional_view: Option<Box<HirObjectView>>,
    pub array_element: Option<Box<super::HirArrayElementPlace>>,
    pub field: FieldId,
    pub span: Span,
}
