//! Name-resolved, but not yet type-checked, program representation.

use crate::{
    function_table::{DenseFunctionTable, SparseFunctionTable},
    identity::{BindingId, FunctionId, LocalId, ParameterId},
    literal::NumericLiteralKind,
    source::Span,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedProgram {
    pub declarations: ResolvedFunctionDeclarationTable,
    pub definitions: ResolvedFunctionDefinitionTable,
    /// Function named `main`, selected during resolution. Type checking
    /// validates its signature and diagnoses its absence.
    pub entry_function: Option<FunctionId>,
    pub span: Span,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ResolvedFunctionDeclarationTable {
    entries: DenseFunctionTable<ResolvedFunctionDeclaration>,
}

impl ResolvedFunctionDeclarationTable {
    pub(crate) fn new(entries: Vec<ResolvedFunctionDeclaration>) -> Self {
        Self {
            entries: DenseFunctionTable::new(entries, |declaration| declaration.id),
        }
    }

    pub fn get(&self, id: FunctionId) -> Option<&ResolvedFunctionDeclaration> {
        self.entries.get(id, |declaration| declaration.id)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &ResolvedFunctionDeclaration> {
        self.entries.iter()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[cfg(test)]
    pub(crate) fn entries_mut_for_test(&mut self) -> &mut [ResolvedFunctionDeclaration] {
        self.entries.entries_mut_for_test()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedFunctionDeclaration {
    pub id: FunctionId,
    /// Retained for diagnostics, dumps, and external-linkage selection.
    pub name: String,
    pub name_span: Span,
    pub parameters: Vec<ResolvedParameter>,
    pub return_type: ResolvedType,
    pub linkage: ResolvedFunctionLinkage,
    pub span: Span,
}

impl ResolvedFunctionDeclaration {
    pub fn parameter(&self, id: ParameterId) -> Option<&ResolvedParameter> {
        (id.callable() == self.id.into())
            .then(|| self.parameters.get(id.index()))
            .flatten()
            .filter(|parameter| parameter.id == id)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedFunctionLinkage {
    Internal,
    External { symbol: String },
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ResolvedFunctionDefinitionTable {
    entries: SparseFunctionTable<ResolvedFunctionDefinition>,
}

impl ResolvedFunctionDefinitionTable {
    pub(crate) fn new(entries: Vec<Option<ResolvedFunctionDefinition>>) -> Self {
        Self {
            entries: SparseFunctionTable::new(entries, |definition| definition.function),
        }
    }

    pub fn get(&self, function: FunctionId) -> Option<&ResolvedFunctionDefinition> {
        self.entries.get(function)
    }

    pub fn iter(&self) -> impl Iterator<Item = &ResolvedFunctionDefinition> {
        self.entries.iter()
    }

    pub const fn len(&self) -> usize {
        self.entries.len()
    }

    pub const fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedFunctionDefinition {
    pub function: FunctionId,
    pub locals: Vec<ResolvedLocal>,
    pub body: ResolvedBlock,
    pub span: Span,
}

impl ResolvedFunctionDefinition {
    pub fn local(&self, id: LocalId) -> Option<&ResolvedLocal> {
        (id.callable() == self.function.into())
            .then(|| self.locals.get(id.index()))
            .flatten()
            .filter(|local| local.id == id)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedParameter {
    pub id: ParameterId,
    pub name: String,
    pub name_span: Span,
    pub type_syntax: ResolvedType,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedLocal {
    pub id: LocalId,
    pub name: String,
    pub name_span: Span,
    pub type_syntax: ResolvedType,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolvedTypeKind {
    I64,
    U64,
    U8,
    F64,
    Bool,
    Unit,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedType {
    pub kind: ResolvedTypeKind,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedBlock {
    pub statements: Vec<ResolvedStatement>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedStatement {
    Local(ResolvedLocalDecl),
    Return(ResolvedReturn),
    Expression(ResolvedExpressionStatement),
    Conditional(ResolvedConditional),
    Block(ResolvedBlock),
}

impl ResolvedStatement {
    pub const fn span(&self) -> Span {
        match self {
            Self::Local(statement) => statement.span,
            Self::Return(statement) => statement.span,
            Self::Expression(statement) => statement.span,
            Self::Conditional(statement) => statement.span,
            Self::Block(block) => block.span,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedLocalDecl {
    pub local: LocalId,
    pub initializer: ResolvedExpression,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedReturn {
    pub value: Option<ResolvedExpression>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedExpressionStatement {
    pub expression: ResolvedExpression,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedConditional {
    pub arms: Vec<ResolvedConditionalArm>,
    pub else_block: Option<ResolvedBlock>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedConditionalArm {
    pub condition: ResolvedExpression,
    pub body: ResolvedBlock,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedExpression {
    Binding(ResolvedBindingExpr),
    NumericLiteral(ResolvedNumericLiteralExpr),
    Boolean(ResolvedBooleanExpr),
    Unary(ResolvedUnaryExpr),
    Binary(ResolvedBinaryExpr),
    DirectCall(ResolvedDirectCallExpr),
    Grouped(ResolvedGroupedExpr),
}

impl ResolvedExpression {
    pub const fn span(&self) -> Span {
        match self {
            Self::Binding(expression) => expression.span,
            Self::NumericLiteral(expression) => expression.span,
            Self::Boolean(expression) => expression.span,
            Self::Unary(expression) => expression.span,
            Self::Binary(expression) => expression.span,
            Self::DirectCall(expression) => expression.span,
            Self::Grouped(expression) => expression.span,
        }
    }
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
