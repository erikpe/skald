//! Typed scalar expressions, calls, and their arguments.

use crate::{
    identity::{
        BindingId, CopyConstructorId, FunctionId, InterfaceId, InterfaceRequirementId, MethodId,
        VirtualFamilyId, VirtualSlotId,
    },
    source::Span,
};

use super::{
    object::{
        HirCheckedObjectView, HirFieldPlace, HirMethodReceiver, HirObjectPlace, HirObjectSource,
        HirObjectView, HirSelectedCopyOperation, HirViewTarget,
    },
    HirOptionalOperand, HirOptionalSource, HirPresenceTestKind, HirSharedTransfer, Type,
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
    InterfaceCall {
        receiver: HirInterfaceReceiver,
        target: HirInterfaceCallTarget,
        arguments: Vec<HirCallArgument>,
    },
    TypeTest(HirTypeTest),
    PresenceTest {
        source: HirOptionalOperand,
        kind: HirPresenceTestKind,
    },
    Unwrap(HirOptionalOperand),
    Grouped(Box<HirExpression>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirInterfaceReceiver {
    View(HirObjectView),
    Checked(Box<HirCheckedObjectView>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirTypeTest {
    pub source: HirObjectView,
    pub target: HirViewTarget,
    pub kind: HirTypeTestKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirTypeTestKind {
    StaticSuccess,
    StaticFailure,
    Runtime,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HirInterfaceCallTarget {
    pub interface: InterfaceId,
    pub requirement: InterfaceRequirementId,
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
    Optional {
        source: HirOptionalSource,
        payload: super::HirPrimitiveType,
    },
    ClassOptional(super::HirClassOptionalInitialize),
    OptionalShared(super::HirOptionalSharedInitialize),
    Place(HirObjectPlace),
    View(HirObjectView),
    CheckedView(Box<HirCheckedObjectView>),
    Copy(HirCopyArgument),
    Shared(HirSharedTransfer),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirCopyArgument {
    pub source: HirObjectSource,
    pub operation: HirSelectedCopyOperation<CopyConstructorId>,
    pub span: Span,
}

impl HirCallArgument {
    pub const fn span(&self) -> Span {
        match self {
            Self::Value(expression) => expression.span,
            Self::Optional { source, .. } => source.span(),
            Self::ClassOptional(value) => value.span,
            Self::OptionalShared(value) => value.span,
            Self::Place(place) => place.span(),
            Self::View(view) => view.span,
            Self::CheckedView(view) => view.span,
            Self::Copy(copy) => copy.span,
            Self::Shared(value) => value.span,
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
