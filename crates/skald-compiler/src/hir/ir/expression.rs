//! Typed scalar expressions, calls, and their arguments.

use crate::{
    identity::{BindingId, FunctionId, InitializerId, MethodId, VirtualFamilyId, VirtualSlotId},
    source::Span,
};

use super::{
    object::{
        HirFieldPlace, HirMethodReceiver, HirObjectPlace, HirObjectSource, HirObjectView,
        HirSelectedCopyOperation,
    },
    Type,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirExpression {
    pub kind: HirExpressionKind,
    pub ty: Type,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirExpressionKind {
    Binding(BindingId),
    I64(i64),
    U64(u64),
    U8(u8),
    /// IEEE-754 binary64 payload, kept as raw bits for deterministic HIR.
    F64Bits(u64),
    Boolean(bool),
    Unary {
        operation: HirUnaryOperation,
        operand: Box<HirExpression>,
    },
    Binary {
        operation: HirBinaryOperation,
        left: Box<HirExpression>,
        right: Box<HirExpression>,
    },
    DirectCall {
        function: FunctionId,
        arguments: Vec<HirCallArgument>,
    },
    FieldRead(HirFieldPlace),
    MethodCall {
        receiver: HirMethodReceiver,
        target: HirMethodCallTarget,
        arguments: Vec<HirCallArgument>,
    },
    Grouped(Box<HirExpression>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirMethodCallTarget {
    Direct(MethodId),
    Virtual {
        family: VirtualFamilyId,
        slot: VirtualSlotId,
        selected: MethodId,
    },
}

impl HirMethodCallTarget {
    pub const fn selected(self) -> MethodId {
        match self {
            Self::Direct(method)
            | Self::Virtual {
                selected: method, ..
            } => method,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirCallArgument {
    Value(HirExpression),
    Place(HirObjectPlace),
    View(HirObjectView),
    Copy(HirCopyArgument),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirCopyArgument {
    pub source: HirObjectSource,
    pub operation: HirSelectedCopyOperation<InitializerId>,
    pub span: Span,
}

impl HirCallArgument {
    pub const fn span(&self) -> Span {
        match self {
            Self::Value(expression) => expression.span,
            Self::Place(place) => place.span(),
            Self::View(view) => view.span,
            Self::Copy(copy) => copy.span,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirUnaryOperation {
    NegateI64,
    NegateF64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirBinaryOperation {
    AddI64,
    SubtractI64,
    MultiplyI64,
    AddU64,
    SubtractU64,
    MultiplyU64,
    AddU8,
    SubtractU8,
    MultiplyU8,
    AddF64,
    SubtractF64,
    MultiplyF64,
}
