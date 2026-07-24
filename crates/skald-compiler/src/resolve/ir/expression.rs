//! Name-resolved expression trees.

use crate::{
    identity::{
        BindingId, ClassId, FieldId, FunctionId, InterfaceId, InterfaceRequirementId, MethodId,
    },
    literal::NumericLiteralKind,
    source::Span,
};

use super::object_place::ResolvedObjectReceiver;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedExpression {
    Binding(ResolvedBindingExpr),
    NumericLiteral(ResolvedNumericLiteralExpr),
    Boolean(ResolvedBooleanExpr),
    Unary(ResolvedUnaryExpr),
    Binary(ResolvedBinaryExpr),
    TypeTest(ResolvedTypeTestExpr),
    ObjectCast(ResolvedObjectCastExpr),
    DirectCall(ResolvedDirectCallExpr),
    Grouped(ResolvedGroupedExpr),
    FieldAccess(ResolvedFieldAccessExpr),
    MethodCall(ResolvedMethodCallExpr),
    InterfaceCall(ResolvedInterfaceCallExpr),
    Construct(ResolvedConstructExpr),
}

impl ResolvedExpression {
    pub const fn span(&self) -> Span {
        match self {
            Self::Binding(expression) => expression.span,
            Self::NumericLiteral(expression) => expression.span,
            Self::Boolean(expression) => expression.span,
            Self::Unary(expression) => expression.span,
            Self::Binary(expression) => expression.span,
            Self::TypeTest(expression) => expression.span,
            Self::ObjectCast(expression) => expression.span,
            Self::DirectCall(expression) => expression.span,
            Self::Grouped(expression) => expression.span,
            Self::FieldAccess(expression) => expression.span,
            Self::MethodCall(expression) => expression.span,
            Self::InterfaceCall(expression) => expression.span,
            Self::Construct(expression) => expression.span,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedObjectCastExpr {
    pub source: Box<ResolvedExpression>,
    pub target: super::ResolvedType,
    pub target_mode: ResolvedObjectCastTargetMode,
    pub target_span: Span,
    pub span: Span,
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedFieldAccessExpr {
    pub receiver: ResolvedObjectReceiver,
    pub field: FieldId,
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
pub struct ResolvedConstructExpr {
    pub class: ClassId,
    pub callee_span: Span,
    pub arguments: Vec<ResolvedExpression>,
    pub span: Span,
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedBinaryExpr {
    pub left: Box<ResolvedExpression>,
    pub operator: ResolvedBinaryOperator,
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
