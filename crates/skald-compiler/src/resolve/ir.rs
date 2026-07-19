//! Name-resolved, but not yet type-checked, program representation.

use std::fmt;

use crate::source::Span;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FunctionId(usize);

impl FunctionId {
    pub const fn index(self) -> usize {
        self.0
    }

    pub(crate) const fn new(index: usize) -> Self {
        Self(index)
    }
}

impl fmt::Display for FunctionId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "f{}", self.index())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ParameterId {
    function: FunctionId,
    index: usize,
}

impl ParameterId {
    pub const fn function(self) -> FunctionId {
        self.function
    }

    pub const fn index(self) -> usize {
        self.index
    }

    pub(crate) const fn new(function: FunctionId, index: usize) -> Self {
        Self { function, index }
    }
}

impl fmt::Display for ParameterId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}:p{}", self.function(), self.index())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LocalId {
    function: FunctionId,
    index: usize,
}

impl LocalId {
    pub const fn function(self) -> FunctionId {
        self.function
    }

    pub const fn index(self) -> usize {
        self.index
    }

    pub(crate) const fn new(function: FunctionId, index: usize) -> Self {
        Self { function, index }
    }
}

impl fmt::Display for LocalId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}:l{}", self.function(), self.index())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BindingId {
    Parameter(ParameterId),
    Local(LocalId),
}

impl BindingId {
    pub const fn function(self) -> FunctionId {
        match self {
            Self::Parameter(id) => id.function(),
            Self::Local(id) => id.function(),
        }
    }
}

impl fmt::Display for BindingId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parameter(id) => id.fmt(formatter),
            Self::Local(id) => id.fmt(formatter),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedProgram {
    pub declarations: ResolvedFunctionDeclarationTable,
    pub definitions: ResolvedFunctionDefinitionTable,
    /// Function named `main`, selected during resolution. M4 validates its
    /// signature and diagnoses its absence.
    pub entry_function: Option<FunctionId>,
    pub span: Span,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ResolvedFunctionDeclarationTable {
    entries: Vec<ResolvedFunctionDeclaration>,
}

impl ResolvedFunctionDeclarationTable {
    pub(crate) fn new(entries: Vec<ResolvedFunctionDeclaration>) -> Self {
        debug_assert!(
            entries
                .iter()
                .enumerate()
                .all(|(index, declaration)| declaration.id.index() == index),
            "function declaration table must be dense and ordered by ID"
        );
        Self { entries }
    }

    pub fn get(&self, id: FunctionId) -> Option<&ResolvedFunctionDeclaration> {
        self.entries
            .get(id.index())
            .filter(|declaration| declaration.id == id)
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
        &mut self.entries
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
        (id.function() == self.id)
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
    entries: Vec<Option<ResolvedFunctionDefinition>>,
    definition_count: usize,
}

impl ResolvedFunctionDefinitionTable {
    pub(crate) fn new(entries: Vec<Option<ResolvedFunctionDefinition>>) -> Self {
        debug_assert!(entries.iter().enumerate().all(|(index, definition)| {
            definition
                .as_ref()
                .is_none_or(|definition| definition.function.index() == index)
        }));
        let definition_count = entries.iter().flatten().count();
        Self {
            entries,
            definition_count,
        }
    }

    pub fn get(&self, function: FunctionId) -> Option<&ResolvedFunctionDefinition> {
        self.entries.get(function.index())?.as_ref()
    }

    pub fn iter(&self) -> impl Iterator<Item = &ResolvedFunctionDefinition> {
        self.entries.iter().flatten()
    }

    pub const fn len(&self) -> usize {
        self.definition_count
    }

    pub const fn is_empty(&self) -> bool {
        self.definition_count == 0
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
        (id.function() == self.function)
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
    Block(ResolvedBlock),
}

impl ResolvedStatement {
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
pub enum ResolvedExpression {
    Binding(ResolvedBindingExpr),
    Integer(ResolvedIntegerExpr),
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
            Self::Integer(expression) => expression.span,
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
pub struct ResolvedIntegerExpr {
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
