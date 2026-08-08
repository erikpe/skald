//! Name-resolved expression trees.

use crate::{
    identity::{
        BindingId, ClassId, FieldId, FunctionId, InterfaceId, InterfaceRequirementId,
        LiteralDataId, MethodId, StaticFieldId,
    },
    literal::NumericLiteralKind,
    source::Span,
};

use super::object_place::ResolvedObjectReceiver;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedExpression {
    Absent(ResolvedAbsentExpr),
    Binding(ResolvedBindingExpr),
    NumericLiteral(ResolvedNumericLiteralExpr),
    ByteLiteral(ResolvedByteLiteralExpr),
    StringLiteral(ResolvedStringLiteralExpr),
    Boolean(ResolvedBooleanExpr),
    Unary(ResolvedUnaryExpr),
    Dereference(ResolvedDereferenceExpr),
    Binary(ResolvedBinaryExpr),
    Logical(ResolvedLogicalExpr),
    TypeTest(ResolvedTypeTestExpr),
    PresenceTest(ResolvedPresenceTestExpr),
    Unwrap(ResolvedUnwrapExpr),
    PrimitiveCast(ResolvedPrimitiveCastExpr),
    ObjectCast(ResolvedObjectCastExpr),
    Allocation(ResolvedAllocationExpr),
    ArrayConstruction(Box<ResolvedArrayConstructionExpr>),
    ArrayLength(Box<ResolvedArrayLengthExpr>),
    DirectCall(ResolvedDirectCallExpr),
    StaticCall(ResolvedStaticCallExpr),
    Grouped(ResolvedGroupedExpr),
    FieldAccess(ResolvedFieldAccessExpr),
    StaticFieldAccess(ResolvedStaticFieldAccessExpr),
    ArrayProjection(Box<ResolvedArrayProjectionExpr>),
    MethodCall(ResolvedMethodCallExpr),
    InterfaceCall(ResolvedInterfaceCallExpr),
    Construct(ResolvedConstructExpr),
}

impl ResolvedExpression {
    pub const fn span(&self) -> Span {
        match self {
            Self::Absent(expression) => expression.span,
            Self::Binding(expression) => expression.span,
            Self::NumericLiteral(expression) => expression.span,
            Self::ByteLiteral(expression) => expression.span,
            Self::StringLiteral(expression) => expression.span,
            Self::Boolean(expression) => expression.span,
            Self::Unary(expression) => expression.span,
            Self::Dereference(expression) => expression.span,
            Self::Binary(expression) => expression.span,
            Self::Logical(expression) => expression.span,
            Self::TypeTest(expression) => expression.span,
            Self::PresenceTest(expression) => expression.span,
            Self::Unwrap(expression) => expression.span,
            Self::PrimitiveCast(expression) => expression.span,
            Self::ObjectCast(expression) => expression.span,
            Self::Allocation(expression) => expression.span,
            Self::ArrayConstruction(expression) => expression.span,
            Self::ArrayLength(expression) => expression.span,
            Self::DirectCall(expression) => expression.span,
            Self::StaticCall(expression) => expression.span,
            Self::Grouped(expression) => expression.span,
            Self::FieldAccess(expression) => expression.span,
            Self::StaticFieldAccess(expression) => expression.span,
            Self::ArrayProjection(expression) => expression.span,
            Self::MethodCall(expression) => expression.span,
            Self::InterfaceCall(expression) => expression.span,
            Self::Construct(expression) => expression.span,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedArrayLengthExpr {
    pub receiver: Box<ResolvedExpression>,
    pub operator: ResolvedArrayLengthOperator,
    pub member_span: Span,
    pub arguments: Vec<ResolvedExpression>,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolvedArrayLengthOperator {
    Ordinary { dot_span: Span },
    Shared { arrow_span: Span },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedArrayConstructionExpr {
    pub new_span: Option<Span>,
    pub array_type: super::ResolvedType,
    pub arguments: ResolvedArrayConstructionArguments,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedArrayElementList {
    pub left_brace_span: Span,
    pub elements: Vec<ResolvedExpression>,
    pub comma_spans: Vec<Span>,
    pub right_brace_span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedArrayConstructionArguments {
    Empty {
        left_paren_span: Span,
        right_paren_span: Span,
    },
    Length {
        left_paren_span: Span,
        length: Box<ResolvedExpression>,
        right_paren_span: Span,
    },
    Copy {
        left_paren_span: Span,
        copy_span: Span,
        source: Box<ResolvedExpression>,
        right_paren_span: Span,
    },
    Elements(ResolvedArrayElementList),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedArrayProjectionExpr {
    pub receiver: Box<ResolvedExpression>,
    pub operator: ResolvedArrayProjectionOperator,
    pub bounds: ResolvedArrayProjectionBounds,
    pub right_bracket_span: Span,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolvedArrayProjectionOperator {
    Ordinary {
        left_bracket_span: Span,
    },
    Shared {
        arrow_span: Span,
        left_bracket_span: Span,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedArrayProjectionBounds {
    Index(Box<ResolvedExpression>),
    Slice {
        start: Option<Box<ResolvedExpression>>,
        colon_span: Span,
        end: Option<Box<ResolvedExpression>>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolvedAbsentExpr {
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolvedByteLiteralExpr {
    pub value: u8,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolvedStringLiteralExpr {
    pub data: LiteralDataId,
    pub class: ClassId,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedPresenceTestExpr {
    pub source: Box<ResolvedExpression>,
    pub kind: ResolvedPresenceTestKind,
    pub is_span: Span,
    pub target_span: Span,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolvedPresenceTestKind {
    Some,
    None,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedUnwrapExpr {
    pub source: Box<ResolvedExpression>,
    pub bang_span: Span,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedDereferenceExpr {
    pub source: Box<ResolvedExpression>,
    pub target: super::ResolvedSharedTarget,
    pub operator: ResolvedDereferenceOperator,
    pub operator_span: Span,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolvedDereferenceOperator {
    Star,
    Arrow,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedAllocationExpr {
    pub class: ClassId,
    pub new_span: Span,
    pub target_span: Span,
    pub mode: ResolvedConstructionMode,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedObjectCastExpr {
    pub source: Box<ResolvedExpression>,
    pub target: super::ResolvedType,
    pub target_mode: ResolvedObjectCastTargetMode,
    pub target_span: Span,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedPrimitiveCastExpr {
    pub target: ResolvedPrimitiveType,
    pub target_span: Span,
    pub source: Box<ResolvedExpression>,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolvedPrimitiveType {
    I64,
    U64,
    U8,
    F64,
    Bool,
}

impl ResolvedPrimitiveType {
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolvedObjectCastTargetMode {
    Plain,
    Shared { shared_span: Span },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedTypeTestExpr {
    pub source: Box<ResolvedExpression>,
    pub target: super::ResolvedType,
    pub target_span: Span,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedInterfaceCallExpr {
    pub receiver: ResolvedInterfaceReceiver,
    pub interface: InterfaceId,
    pub requirement: InterfaceRequirementId,
    pub receiver_span: Span,
    pub member_span: Span,
    pub arguments: Vec<ResolvedExpression>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedInterfaceReceiver {
    Binding { binding: BindingId, span: Span },
    Cast(Box<ResolvedObjectCastExpr>),
    Dereference(Box<ResolvedDereferenceExpr>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedFieldAccessExpr {
    pub receiver: ResolvedObjectReceiver,
    pub field: FieldId,
    pub member_span: Span,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolvedStaticFieldAccessExpr {
    pub field: StaticFieldId,
    pub member_span: Span,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedMethodCallExpr {
    pub receiver: ResolvedObjectReceiver,
    pub method: MethodId,
    pub member_span: Span,
    pub arguments: Vec<ResolvedExpression>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedStaticCallExpr {
    pub method: MethodId,
    pub member_span: Span,
    pub arguments: Vec<ResolvedExpression>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedConstructExpr {
    pub class: ClassId,
    pub callee_span: Span,
    pub mode: ResolvedConstructionMode,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedConstructionMode {
    Initialize {
        arguments: Vec<ResolvedExpression>,
    },
    Copy {
        copy_span: Span,
        source: Box<ResolvedExpression>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedBindingExpr {
    pub binding: BindingId,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedNumericLiteralExpr {
    pub kind: NumericLiteralKind,
    pub spelling: String,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolvedBooleanExpr {
    pub value: bool,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolvedUnaryOperator {
    Negate,
    LogicalNot,
    BitwiseComplement,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedUnaryExpr {
    pub operator: ResolvedUnaryOperator,
    pub operator_span: Span,
    pub operand: Box<ResolvedExpression>,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolvedBinaryOperator {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
    ShiftLeft,
    ShiftRight,
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
    Equal,
    NotEqual,
    LessThan,
    LessEqual,
    GreaterThan,
    GreaterEqual,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedBinaryExpr {
    pub left: Box<ResolvedExpression>,
    pub operator: ResolvedBinaryOperator,
    pub operator_span: Span,
    pub right: Box<ResolvedExpression>,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolvedLogicalOperator {
    And,
    Or,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedLogicalExpr {
    pub left: Box<ResolvedExpression>,
    pub operator: ResolvedLogicalOperator,
    pub operator_span: Span,
    pub right: Box<ResolvedExpression>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedDirectCallExpr {
    pub function: FunctionId,
    pub callee_span: Span,
    pub arguments: Vec<ResolvedExpression>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedGroupedExpr {
    pub expression: Box<ResolvedExpression>,
    pub span: Span,
}
