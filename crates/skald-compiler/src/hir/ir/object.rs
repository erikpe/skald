//! Typed object construction, copying, calls, and place operations.

use crate::{
    identity::{
        BindingId, ClassId, CopyAssignmentId, CopyConstructorId, FieldId, FunctionId,
        InitializerId, InterfaceId, MethodId,
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
    /// Direct final fields this synthesized assignment may update. This is
    /// empty for copy construction.
    pub final_fields: Vec<FieldId>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HirBaseCopy<I> {
    pub base: ClassId,
    pub operation: HirSelectedCopyOperation<I>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirSynthesizedFieldCopy<I> {
    Scalar {
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
    Optional {
        field: FieldId,
        optional: crate::identity::OptionalTypeId,
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
            Self::Scalar { field }
            | Self::OptionalPrimitive { field, .. }
            | Self::OptionalClass { field, .. }
            | Self::Shared { field }
            | Self::OptionalShared { field, .. }
            | Self::Optional { field, .. }
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
    IndirectCall(Box<super::HirIndirectCall>),
    StringLiteral(super::HirStringLiteral),
}

impl HirObjectProducer {
    pub const fn class(&self) -> ClassId {
        match self {
            Self::Construct(construction) => construction.class,
            Self::Call(call) => call.class,
            Self::IndirectCall(call) => match call.result {
                super::Type::Class(class) => class,
                _ => panic!("object-producing indirect call must have a class result"),
            },
            Self::StringLiteral(literal) => literal.class,
        }
    }

    pub const fn span(&self) -> Span {
        match self {
            Self::Construct(construction) => construction.span,
            Self::Call(call) => call.span,
            Self::IndirectCall(call) => call.span,
            Self::StringLiteral(literal) => literal.span,
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
    Static(MethodId),
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
    /// An exact-class object stored in a receiver-free static slot.
    Static {
        place: super::HirStaticPlace,
        class: ClassId,
    },
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
            Self::Static { class, .. } => *class,
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
            Self::Static { place, .. } => place.span,
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

/// Copy assignment into an already-published exact-class static slot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirStaticCopyAssignment {
    pub destination: super::HirStaticPlace,
    pub class: ClassId,
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
    /// A receiver-free exact object stored in a mutable static slot.
    Static {
        place: super::HirStaticPlace,
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

/// One exhaustive provenance carrier for a class member receiver.
///
/// Stable places retain their selected subobject and complete-object origin.
/// Checked casts and array elements remain distinct because their guards and
/// addressing are lowered specially. Every other non-place receiver uses the
/// common object-view path, including shared, optional-backed, and produced
/// exact-class receivers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirObjectReceiver {
    Place {
        place: HirObjectPlace,
        origin: Box<HirObjectOrigin>,
    },
    Checked {
        /// Retained to preserve the selected member path in deterministic HIR
        /// inspection; executable lowering consumes `view`.
        place: HirObjectPlace,
        origin: Box<HirObjectOrigin>,
        view: Box<HirCheckedObjectView>,
    },
    View {
        view: Box<HirObjectView>,
        /// Existing shared and optional member paths retain their historical
        /// inspection path without making it executable provenance. A future
        /// produced view has no source binding and leaves this absent.
        inspection_place: Option<Box<HirObjectPlace>>,
    },
    ArrayElement {
        element: Box<super::HirArrayElementPlace>,
        /// Projections after the checked element remain explicit for field
        /// addressing and HIR inspection.
        place: HirObjectPlace,
        origin: Box<HirObjectOrigin>,
    },
}

impl HirObjectReceiver {
    pub fn access(&self) -> HirAccess {
        match self {
            Self::Place { place, .. }
            | Self::Checked { place, .. }
            | Self::ArrayElement { place, .. } => place.access,
            Self::View { view, .. } => view.access,
        }
    }

    /// Returns the retained source-path view used for deterministic inspection
    /// and existing-place checks. Executable lowering matches the carrier and
    /// never treats this as provenance for `View`.
    pub fn inspection_place(&self) -> Option<&HirObjectPlace> {
        match self {
            Self::Place { place, .. }
            | Self::Checked { place, .. }
            | Self::ArrayElement { place, .. } => Some(place),
            Self::View {
                inspection_place, ..
            } => inspection_place.as_deref(),
        }
    }

    pub fn origin(&self) -> &HirObjectOrigin {
        match self {
            Self::Place { origin, .. }
            | Self::Checked { origin, .. }
            | Self::ArrayElement { origin, .. } => origin,
            Self::View { view, .. } => &view.origin,
        }
    }
}

pub type HirMethodReceiver = HirObjectReceiver;

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
    /// An exact object selected from receiver-free static storage.
    Static {
        place: super::HirStaticPlace,
        projections: Vec<crate::object_path::ObjectProjection>,
    },
    /// An exact object selected from checked array storage. The element
    /// carrier preserves its owner anchor and bounds-checking semantics.
    ArrayElement(Box<super::HirArrayElementPlace>),
    Produced {
        producer: Box<HirObjectProducer>,
        /// Static base projections from the complete produced class to the
        /// view's class target. Interface and `Obj` views need no projection.
        projections: Vec<crate::object_path::ObjectProjection>,
    },
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
    /// A present object selected through an immutable polymorphic optional
    /// box. MIR establishes the owner anchor before its presence guard.
    OptionalBoxPayload {
        view: Box<super::HirOptionalBoxObjectView>,
        projections: Vec<crate::object_path::ObjectProjection>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirFieldPlace {
    pub receiver: HirObjectReceiver,
    pub field: FieldId,
    /// Present only when this place is the complete destination of a checked
    /// field replacement. Reads and initialization destinations carry no
    /// replacement authorization.
    pub write_authorization: Option<HirFieldWriteAuthorization>,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirFieldWriteAuthorization {
    Mutable,
    DeclaringClassCell,
    /// Exact user copy-assignment lifecycle authorized to update this direct
    /// declaring-class final field.
    DeclaringClassFinalAssignment(CopyAssignmentId),
}
