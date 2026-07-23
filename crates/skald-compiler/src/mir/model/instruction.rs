//! MIR instructions, calls, and argument forms.

use crate::{
    identity::{
        ClassId, CopyAssignmentId, FunctionId, InitializerId, InterfaceId, InterfaceRequirementId,
        MethodId, VirtualFamilyId, VirtualSlotId,
    },
    source::Span,
};

use super::{
    declarations::MirSelectedCopyOperation,
    definition::MirAliasAccess,
    ids::{StorageId, ValueId},
    value::{MirPlace, MirRvalue},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirInstruction {
    Assign(MirAssignment),
    Call(MirCall),
    Cleanup(MirCleanup),
    Initialize(MirInitialize),
    Store(MirStore),
    CopyConstruct(MirCopyConstruction),
    CopyAssign(MirCopyAssignment),
    EndFullExpression(MirEndFullExpression),
}

impl MirInstruction {
    pub const fn span(&self) -> Span {
        match self {
            Self::Assign(instruction) => instruction.span,
            Self::Call(instruction) => instruction.span,
            Self::Cleanup(instruction) => instruction.span,
            Self::Initialize(instruction) => instruction.span,
            Self::Store(instruction) => instruction.span,
            Self::CopyConstruct(instruction) => instruction.span,
            Self::CopyAssign(instruction) => instruction.span,
            Self::EndFullExpression(instruction) => instruction.span,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirCleanup {
    pub destination: MirPlace,
    pub target: ClassId,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirAssignment {
    pub result: ValueId,
    pub rvalue: MirRvalue,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirStore {
    pub destination: MirPlace,
    pub value: ValueId,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirInitialize {
    pub destination: MirPlace,
    pub target: InitializerId,
    pub arguments: Vec<MirArgument>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirCopyConstruction {
    pub destination: MirPlace,
    pub source: MirPlace,
    pub class: ClassId,
    pub operation: MirSelectedCopyOperation<InitializerId>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirCopyAssignment {
    pub destination: MirPlace,
    pub source: MirPlace,
    pub class: ClassId,
    pub operation: MirSelectedCopyOperation<CopyAssignmentId>,
    pub span: Span,
}

/// Ends one object-producing full expression and destroys its temporaries in
/// reverse completion order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirEndFullExpression {
    pub temporaries: Vec<MirCleanup>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirCall {
    pub target: MirCallTarget,
    pub receiver: Option<MirCallReceiver>,
    pub arguments: Vec<MirArgument>,
    pub result: Option<ValueId>,
    /// Caller-owned uninitialized storage for a class result.
    pub destination: Option<MirPlace>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirCallReceiver {
    Method(MirMethodReceiver),
    Interface(MirObjectView),
}

impl MirCallReceiver {
    pub fn as_method(&self) -> Option<&MirMethodReceiver> {
        match self {
            Self::Method(receiver) => Some(receiver),
            Self::Interface(_) => None,
        }
    }

    pub fn as_method_mut(&mut self) -> Option<&mut MirMethodReceiver> {
        match self {
            Self::Method(receiver) => Some(receiver),
            Self::Interface(_) => None,
        }
    }

    pub fn as_interface(&self) -> Option<&MirObjectView> {
        match self {
            Self::Method(_) => None,
            Self::Interface(view) => Some(view),
        }
    }

    pub fn as_interface_mut(&mut self) -> Option<&mut MirObjectView> {
        match self {
            Self::Method(_) => None,
            Self::Interface(view) => Some(view),
        }
    }
}

impl From<MirMethodReceiver> for MirCallReceiver {
    fn from(receiver: MirMethodReceiver) -> Self {
        Self::Method(receiver)
    }
}

impl From<MirObjectView> for MirCallReceiver {
    fn from(view: MirObjectView) -> Self {
        Self::Interface(view)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirArgument {
    Value(ValueId),
    Place(MirPlace),
    View(MirObjectView),
    /// A complete live caller destination transferred to the corresponding
    /// class value parameter for the duration of the call.
    OwnedPlace(MirPlace),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirViewTarget {
    Class(ClassId),
    Interface(InterfaceId),
    Obj,
}

impl MirViewTarget {
    pub const fn ty(self) -> super::value::MirType {
        match self {
            Self::Class(class) => super::value::MirType::Class(class),
            Self::Interface(interface) => super::value::MirType::Interface(interface),
            Self::Obj => super::value::MirType::Obj,
        }
    }
}

/// One non-owning static conversion at an alias call boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirObjectView {
    pub source: MirPlace,
    pub origin: Box<MirObjectOrigin>,
    pub target: MirViewTarget,
    pub access: MirAliasAccess,
    pub span: Span,
}

impl From<ValueId> for MirArgument {
    fn from(value: ValueId) -> Self {
        Self::Value(value)
    }
}

impl From<MirPlace> for MirArgument {
    fn from(place: MirPlace) -> Self {
        Self::Place(place)
    }
}

impl MirArgument {
    pub fn values(values: impl IntoIterator<Item = ValueId>) -> Vec<Self> {
        values.into_iter().map(Self::Value).collect()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirCallTarget {
    Direct(FunctionId),
    Method(MirMethodCallTarget),
    Interface(MirInterfaceCallTarget),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MirInterfaceCallTarget {
    pub interface: InterfaceId,
    pub requirement: InterfaceRequirementId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirMethodCallTarget {
    Direct(MethodId),
    Virtual {
        family: VirtualFamilyId,
        slot: VirtualSlotId,
        selected: MethodId,
    },
}

impl MirMethodCallTarget {
    pub const fn selected(self) -> MethodId {
        match self {
            Self::Direct(method)
            | Self::Virtual {
                selected: method, ..
            } => method,
        }
    }
}

/// Static receiver selection plus the complete-object provenance needed to
/// preserve dynamic dispatch through direct and virtual calls.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirMethodReceiver {
    pub place: MirPlace,
    pub origin: Box<MirObjectOrigin>,
}

impl MirMethodReceiver {
    pub fn exact(place: MirPlace, dynamic_class: ClassId) -> Self {
        Self {
            origin: Box::new(MirObjectOrigin::Exact {
                complete: place.clone(),
                dynamic_class,
            }),
            place,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirObjectOrigin {
    Exact {
        complete: MirPlace,
        dynamic_class: ClassId,
    },
    Forwarded {
        carrier: StorageId,
        static_target: MirViewTarget,
        access: MirAliasAccess,
        dispatch_limit: Option<ClassId>,
        span: Span,
    },
}
