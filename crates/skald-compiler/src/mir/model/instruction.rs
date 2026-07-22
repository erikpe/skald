//! MIR instructions, calls, and argument forms.

use crate::{
    identity::{ClassId, CopyAssignmentId, FunctionId, InitializerId, MethodId},
    source::Span,
};

use super::{
    declarations::MirSelectedCopyOperation,
    ids::ValueId,
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
    pub receiver: Option<MirPlace>,
    pub arguments: Vec<MirArgument>,
    pub result: Option<ValueId>,
    /// Caller-owned uninitialized storage for a class result.
    pub destination: Option<MirPlace>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirArgument {
    Value(ValueId),
    Place(MirPlace),
    /// A complete live caller destination transferred to the corresponding
    /// class value parameter for the duration of the call.
    OwnedPlace(MirPlace),
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
    Method(MethodId),
}
