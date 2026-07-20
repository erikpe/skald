//! Fully typed HIR consumed by MIR lowering.

use crate::{
    resolve::{BindingId, FunctionId, LocalId, ParameterId},
    source::Span,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Type {
    I64,
    U64,
    U8,
    Bool,
    Unit,
}

impl Type {
    pub const fn name(self) -> &'static str {
        match self {
            Self::I64 => "i64",
            Self::U64 => "u64",
            Self::U8 => "u8",
            Self::Bool => "bool",
            Self::Unit => "unit",
        }
    }

    /// Returns the English indefinite article used before this type's name in
    /// diagnostics.
    pub const fn indefinite_article(self) -> &'static str {
        match self {
            Self::I64 => "an",
            Self::U64 | Self::U8 | Self::Bool | Self::Unit => "a",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirProgram {
    pub declarations: HirFunctionDeclarationTable,
    pub definitions: HirFunctionDefinitionTable,
    pub entry_function: FunctionId,
    pub span: Span,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HirFunctionDeclarationTable {
    entries: Vec<HirFunctionDeclaration>,
}

impl HirFunctionDeclarationTable {
    pub(crate) fn new(entries: Vec<HirFunctionDeclaration>) -> Self {
        debug_assert!(
            entries
                .iter()
                .enumerate()
                .all(|(index, declaration)| declaration.id.index() == index),
            "HIR function declaration table must be dense and ordered by ID"
        );
        Self { entries }
    }

    pub fn get(&self, id: FunctionId) -> Option<&HirFunctionDeclaration> {
        self.entries
            .get(id.index())
            .filter(|declaration| declaration.id == id)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &HirFunctionDeclaration> {
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
pub struct HirFunctionDeclaration {
    pub id: FunctionId,
    pub name: String,
    pub name_span: Span,
    pub parameters: Vec<HirParameter>,
    pub return_type: Type,
    pub linkage: HirFunctionLinkage,
    pub span: Span,
}

impl HirFunctionDeclaration {
    pub fn parameter(&self, id: ParameterId) -> Option<&HirParameter> {
        (id.function() == self.id)
            .then(|| self.parameters.get(id.index()))
            .flatten()
            .filter(|parameter| parameter.id == id)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirFunctionLinkage {
    Internal,
    External { symbol: String },
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HirFunctionDefinitionTable {
    entries: Vec<Option<HirFunctionDefinition>>,
    definition_count: usize,
}

impl HirFunctionDefinitionTable {
    pub(crate) fn new(entries: Vec<Option<HirFunctionDefinition>>) -> Self {
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

    pub fn get(&self, function: FunctionId) -> Option<&HirFunctionDefinition> {
        self.entries.get(function.index())?.as_ref()
    }

    pub fn iter(&self) -> impl Iterator<Item = &HirFunctionDefinition> {
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
pub struct HirFunctionDefinition {
    pub function: FunctionId,
    pub locals: Vec<HirLocal>,
    pub body: HirBlock,
    pub span: Span,
}

impl HirFunctionDefinition {
    pub fn local(&self, id: LocalId) -> Option<&HirLocal> {
        (id.function() == self.function)
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
    Call(HirCallStatement),
    Conditional(HirConditional),
    Block(HirBlock),
}

impl HirStatement {
    pub const fn span(&self) -> Span {
        match self {
            Self::Local(statement) => statement.span,
            Self::Return(statement) => statement.span,
            Self::Call(statement) => statement.span,
            Self::Conditional(statement) => statement.span,
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
    pub value: Option<HirExpression>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirCallStatement {
    pub call: HirExpression,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirConditional {
    pub arms: Vec<HirConditionalArm>,
    pub else_block: Option<HirBlock>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirConditionalArm {
    pub condition: HirExpression,
    pub body: HirBlock,
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
    I64(i64),
    U64(u64),
    U8(u8),
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
    AddU64,
    SubtractU64,
    MultiplyU64,
    AddU8,
    SubtractU8,
    MultiplyU8,
}
