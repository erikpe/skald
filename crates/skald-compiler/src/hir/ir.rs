//! Fully typed HIR consumed by MIR lowering.

use crate::{
    resolve::{BindingId, FunctionId, LocalId, ParameterId},
    source::Span,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Type {
    I64,
}

impl Type {
    pub const fn name(self) -> &'static str {
        match self {
            Self::I64 => "i64",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirProgram {
    pub functions: HirFunctionTable,
    pub entry_function: FunctionId,
    pub span: Span,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HirFunctionTable {
    entries: Vec<HirFunction>,
}

impl HirFunctionTable {
    pub(crate) fn new(entries: Vec<HirFunction>) -> Self {
        debug_assert!(
            entries
                .iter()
                .enumerate()
                .all(|(index, function)| function.id.index() == index),
            "HIR function table must be dense and ordered by ID"
        );
        Self { entries }
    }

    pub fn get(&self, id: FunctionId) -> Option<&HirFunction> {
        self.entries
            .get(id.index())
            .filter(|function| function.id == id)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &HirFunction> {
        self.entries.iter()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirFunction {
    pub id: FunctionId,
    pub name: String,
    pub name_span: Span,
    pub parameters: Vec<HirParameter>,
    pub return_type: Type,
    pub locals: Vec<HirLocal>,
    pub body: HirBlock,
    pub span: Span,
}

impl HirFunction {
    pub fn parameter(&self, id: ParameterId) -> Option<&HirParameter> {
        (id.function() == self.id)
            .then(|| self.parameters.get(id.index()))
            .flatten()
            .filter(|parameter| parameter.id == id)
    }

    pub fn local(&self, id: LocalId) -> Option<&HirLocal> {
        (id.function() == self.id)
            .then(|| self.locals.get(id.index()))
            .flatten()
            .filter(|local| local.id == id)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirParameter {
    pub id: ParameterId,
    pub name: String,
    pub name_span: Span,
    pub ty: Type,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirLocal {
    pub id: LocalId,
    pub name: String,
    pub name_span: Span,
    pub ty: Type,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirBlock {
    pub statements: Vec<HirStatement>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirStatement {
    Local(HirLocalDecl),
    Return(HirReturn),
    Block(HirBlock),
}

impl HirStatement {
    pub const fn span(&self) -> Span {
        match self {
            Self::Local(statement) => statement.span,
            Self::Return(statement) => statement.span,
            Self::Block(block) => block.span,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirLocalDecl {
    pub local: LocalId,
    pub initializer: HirExpression,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirReturn {
    pub value: HirExpression,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirExpression {
    pub kind: HirExpressionKind,
    pub ty: Type,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirExpressionKind {
    Binding(BindingId),
    Integer(i64),
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
        arguments: Vec<HirExpression>,
    },
    Grouped(Box<HirExpression>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirUnaryOperation {
    NegateI64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirBinaryOperation {
    AddI64,
    SubtractI64,
    MultiplyI64,
}
