//! Source-shaped AST for the first vertical slice.

use crate::source::Span;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompilationUnit {
    pub functions: Vec<FunctionDecl>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionDecl {
    pub name: Name,
    pub parameters: Vec<Parameter>,
    pub return_type: TypeSyntax,
    pub body: Block,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Parameter {
    pub name: Name,
    pub type_syntax: TypeSyntax,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Name {
    pub text: String,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypeKind {
    I64,
    Unit,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeSyntax {
    pub kind: TypeKind,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Block {
    pub statements: Vec<Statement>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Statement {
    Local(LocalDecl),
    Return(ReturnStatement),
    Expression(ExpressionStatement),
    Block(Block),
}

impl Statement {
    pub const fn span(&self) -> Span {
        match self {
            Self::Local(statement) => statement.span,
            Self::Return(statement) => statement.span,
            Self::Expression(statement) => statement.span,
            Self::Block(block) => block.span,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalDecl {
    pub name: Name,
    pub type_syntax: TypeSyntax,
    pub initializer: Expression,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReturnStatement {
    pub value: Option<Expression>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExpressionStatement {
    pub expression: Expression,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Expression {
    Identifier(IdentifierExpr),
    Integer(IntegerExpr),
    Unary(UnaryExpr),
    Binary(BinaryExpr),
    Call(CallExpr),
    Grouped(GroupedExpr),
}

impl Expression {
    pub const fn span(&self) -> Span {
        match self {
            Self::Identifier(expression) => expression.span,
            Self::Integer(expression) => expression.span,
            Self::Unary(expression) => expression.span,
            Self::Binary(expression) => expression.span,
            Self::Call(expression) => expression.span,
            Self::Grouped(expression) => expression.span,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdentifierExpr {
    pub name: Name,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IntegerExpr {
    /// Original decimal spelling. M4 owns conversion and `i64` range checking.
    pub spelling: String,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnaryOperator {
    Negate,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UnaryExpr {
    pub operator: UnaryOperator,
    pub operator_span: Span,
    pub operand: Box<Expression>,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BinaryOperator {
    Add,
    Subtract,
    Multiply,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BinaryExpr {
    pub left: Box<Expression>,
    pub operator: BinaryOperator,
    pub operator_span: Span,
    pub right: Box<Expression>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallExpr {
    pub callee: Box<Expression>,
    pub arguments: Vec<Expression>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupedExpr {
    pub expression: Box<Expression>,
    pub span: Span,
}
